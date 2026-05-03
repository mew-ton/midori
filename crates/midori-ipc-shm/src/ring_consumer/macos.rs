//! macOS 向け shm 確保経路。`shm_open(2)` + `ftruncate(2)` + `mmap(2)`
//! + `shm_unlink(2)`。
//!
//! macOS には Linux の `memfd_create(2)` 相当が無いため、POSIX 共有メモリ
//! オブジェクト (`shm_open`) を使う。`shm_open` は名前付きで kernel 内に
//! 共有メモリオブジェクトを登録し、ファイル記述子を返す。
//!
//! プロトコル:
//!
//! 1. `/midori-<pid>-<nanos>-<counter>` 形式のユニークな名前を生成
//! 2. `O_CREAT | O_EXCL | O_RDWR` で `shm_open` し新規作成
//! 3. `ftruncate` で目標サイズに拡張
//! 4. `mmap` で書き込み可能 / shared でマップ
//! 5. `shm_unlink` で名前空間からエントリを除去（fd / mmap は有効なまま）
//!
//! ステップ 5 の `shm_unlink` は POSIX SHM 慣用イディオムで、後続の起動で
//! 同名衝突が起きないこと、および異常終了時のリーク (kernel 名前空間に
//! ゾンビオブジェクトが残る) を防ぐ。fd が SCM_RIGHTS で driver subprocess に
//! 渡された後に Bridge / driver の双方が fd を close すれば、kernel が
//! オブジェクトを回収する。
//!
//! Linux 経路 (`super::linux`) との API 境界は `create_shm_for_ring` の
//! シグネチャで揃えている: `(MmapMut, OwnedFd)` を返し、上位 dispatch
//! (`super::mod`) で `RingConsumerCore` に包む。

use std::ffi::CString;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use memmap2::{MmapMut, MmapOptions};
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::mman::{shm_open, shm_unlink};
use nix::sys::stat::Mode;

use super::{shared::map_shared_fd, CreateError};

/// 同一プロセス内で連番を発行するためのカウンタ。`shm_open` の名前衝突を
/// `pid + nanos` だけで避けるのは厳密には不十分（同一スレッドで連続呼び出し
/// した場合 `nanos` の解像度を超え得る）なので、追加でモノトニック増加する
/// 16 bit 値を suffix に含めて確実な区別をつける。
static SHM_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);

/// macOS の POSIX SHM オブジェクト名上限 (`PSHMNAMLEN`) は 31 byte。先頭の
/// `/` を含み、`O_CREAT | O_EXCL` で `EEXIST` 以外の error を出さない名前
/// 形式に揃える必要がある。`midori-` prefix + 10 進 PID + nanos 下位 8 桁
/// + 4 桁 counter で最大 30 byte 程度に収める。
const MAX_SHM_NAME_LEN: usize = 31;

/// `slot_size` から `shm_bytes` 分の名前付き shm を確保し、`(mmap, fd)` を
/// 返す。返される `OwnedFd` は driver subprocess に SCM_RIGHTS で渡す前提。
pub(super) fn create_shm_for_ring(shm_bytes: usize) -> Result<(MmapMut, OwnedFd), CreateError> {
    let (owned_fd, name) = open_unique_shm()?;

    // ftruncate でファイルを目標サイズに拡張する。shm_open 直後の
    // オブジェクトはサイズ 0 なので mmap する前に必ず必要。
    let truncate_len = i64::try_from(shm_bytes).map_err(|_| CreateError::Os {
        operation: "ftruncate (size_t→i64 cast)",
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "shm size exceeds i64::MAX",
        ),
    })?;
    if let Err(errno) = nix::unistd::ftruncate(&owned_fd, truncate_len) {
        // ftruncate が失敗した場合、shm_unlink で名前空間からエントリを除去
        // しないと、異常パスで kernel に名前付きエントリが残ってしまう。
        // unlink 自体の失敗は最初の error を覆い隠したくないので無視する。
        let _ = shm_unlink(name.as_c_str());
        return Err(CreateError::Os {
            operation: "ftruncate",
            source: std::io::Error::from(errno),
        });
    }

    let mmap = match map_shared_fd(&owned_fd, shm_bytes, MmapOptions::new()) {
        Ok(mmap) => mmap,
        Err(err) => {
            // mmap 失敗時も同様に kernel 名前空間からエントリを除去する。
            let _ = shm_unlink(name.as_c_str());
            return Err(err);
        }
    };

    // 名前空間からエントリを除去する。fd / mmap は有効なまま継続使用でき、
    // SCM_RIGHTS で渡された driver 側 fd も同一 kernel オブジェクトを指す。
    // これが POSIX SHM の標準的な「open + unlink」パターン。
    if let Err(errno) = shm_unlink(name.as_c_str()) {
        return Err(CreateError::Os {
            operation: "shm_unlink",
            source: std::io::Error::from(errno),
        });
    }

    Ok((mmap, owned_fd))
}

/// `O_CREAT | O_EXCL` で衝突しない名前を見つけるまで `shm_open` を再試行
/// する。`EEXIST` 以外の error は即返す。再試行回数は実用上無限ループに
/// ならない上限 (`MAX_RETRIES`) で打ち切り、外部に `Os` error として返す。
fn open_unique_shm() -> Result<(OwnedFd, CString), CreateError> {
    // 通常 1 回目で成功する。`pid + nanos + counter` の組がぶつかる確率は
    // 極めて低く、複数回ぶつかるなら時計バグ等の異常事態なので 8 回で
    // 打ち切って外部に error を見せる。
    const MAX_RETRIES: u32 = 8;

    let pid = std::process::id();
    let mut last_errno: Option<Errno> = None;

    for _ in 0..MAX_RETRIES {
        let name = build_shm_name(pid);
        let cname = CString::new(name).expect("ASCII shm name has no interior NUL");
        match shm_open(
            cname.as_c_str(),
            OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR,
            Mode::S_IRUSR | Mode::S_IWUSR,
        ) {
            Ok(fd) => return Ok((fd, cname)),
            Err(Errno::EEXIST) => {
                // 衝突: 名前生成側のエントロピー (counter) を進めて再試行。
                last_errno = Some(Errno::EEXIST);
                continue;
            }
            Err(errno) => {
                return Err(CreateError::Os {
                    operation: "shm_open",
                    source: std::io::Error::from(errno),
                });
            }
        }
    }

    Err(CreateError::Os {
        operation: "shm_open (retry exhausted)",
        source: std::io::Error::from(last_errno.unwrap_or(Errno::EEXIST)),
    })
}

/// `/midori-<pid>-<nanos8>-<counter4>` 形式のユニーク名を組み立てる。
///
/// - `pid`: 異なるプロセス間の衝突回避
/// - `nanos8`: `SystemTime::now()` の nanos 下位 8 桁。プロセス内 / 異プロセス
///   両方で時間方向の衝突確率を下げる
/// - `counter4`: 同一プロセス内のモノトニックカウンタ下位 4 桁。同一 nanos
///   で連続生成された場合の決定的な区別をつける
///
/// 全長は最大 30 byte 程度（macOS の `PSHMNAMLEN = 31` 上限内）に収める。
fn build_shm_name(pid: u32) -> String {
    let counter = SHM_NAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    // 各成分を decimal で詰める。16 進だと 1 桁あたりの情報量は多いが
    // PSHMNAMLEN を超えないことを目視で確認しやすい decimal を選ぶ。
    // pid: 最大 7 桁 (Linux 既定 4194304 < 10^7、macOS は 99999) を見込む。
    // nanos: 9 桁を 8 桁に丸める（衝突回避の目的では十分）。
    // counter: 4 桁の wrap で十分（同一 nanos 内で 10000 個生成は非現実）。
    let name = format!(
        "/midori-{pid}-{:08}-{:04}",
        nanos % 100_000_000,
        counter % 10_000,
    );
    debug_assert!(
        name.len() <= MAX_SHM_NAME_LEN,
        "shm name exceeds PSHMNAMLEN: {name} ({} bytes)",
        name.len()
    );
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn it_should_keep_shm_name_within_pshmnamlen() {
        // 最大 PID (32-bit) と最大 counter / nanos でも PSHMNAMLEN を超えない
        // ことを確認する。実環境の PID は通常もっと小さいが、上限耐性を
        // テストしておく。
        let name = build_shm_name(u32::MAX);
        assert!(
            name.len() <= MAX_SHM_NAME_LEN,
            "name too long: {name} ({} bytes)",
            name.len()
        );
        assert!(name.starts_with("/midori-"), "unexpected prefix: {name}");
    }

    #[test]
    fn it_should_yield_unique_names_within_a_burst() {
        // 連続呼び出しで衝突しないこと。counter で確実に区別がつく前提。
        let pid = std::process::id();
        let mut seen = HashSet::new();
        for _ in 0..1024 {
            let name = build_shm_name(pid);
            assert!(seen.insert(name.clone()), "duplicate shm name: {name}");
        }
    }
}
