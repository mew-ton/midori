//! macOS backend: POSIX shared memory (`shm_open(2)` + `ftruncate(2)`
//! + `mmap(2)` + `shm_unlink(2)`) を組み合わせて anonymous shm を確保し、
//! Bridge ↔ driver subprocess 間で fd 経由で受け渡す。
//!
//! 確保プロトコル:
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
//! 同 crate の他 OS backend と `create_shm_for_ring` のシグネチャを揃えて
//! いる: `(MmapMut, OwnedFd)` を返し、上位 dispatch (`super::mod`) で
//! `RingConsumerCore` に包む。

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
/// 形式に揃える必要がある。`build_shm_name` は `/midori-` (8 byte) + 16 進
/// 固定幅 PID 8 桁 + `-` + 16 進固定幅 nanos 8 桁 + `-` + 16 進固定幅
/// counter 4 桁 = 30 byte の固定長で生成し、入力値に依らず常に上限内に
/// 収める（詳細は `build_shm_name` の doc 参照）。
const MAX_SHM_NAME_LEN: usize = 31;

/// `slot_size` から `shm_bytes` 分の名前付き shm を確保し、`(mmap, fd)` を
/// 返す。返される `OwnedFd` は driver subprocess に SCM_RIGHTS で渡す前提。
pub(super) fn create_shm_for_ring(shm_bytes: usize) -> Result<(MmapMut, OwnedFd), CreateError> {
    // ftruncate に渡す i64 への cast を最初にやる。kernel リソース
    // (`shm_open` で作る named shm) を取得する前に validate しておけば、
    // 万一 cast に失敗しても名前付きエントリのリークが起こらない。
    // 実用上 `shm_bytes` (= `page_aligned_shm_size` 出力) は数 MiB 程度
    // なので i64::MAX (≈ 9 EiB) を超えることは事実上ありえないが、契約上
    // 「cast 失敗時もリークさせない」を構造的に保証しておく。
    let truncate_len = i64::try_from(shm_bytes).map_err(|_| CreateError::Os {
        operation: "ftruncate (size_t→i64 cast)",
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "shm size exceeds i64::MAX",
        ),
    })?;

    let (owned_fd, name) = open_unique_shm()?;

    // ftruncate でファイルを目標サイズに拡張する。shm_open 直後の
    // オブジェクトはサイズ 0 なので mmap する前に必ず必要。
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
    //
    // best-effort: unlink が失敗しても、すでに有効な (mmap, fd) を持って
    // いるので caller に対して `Err` を返してリソース全体を捨てる理由は
    // ない。失敗時の症状は kernel 名前空間にゾンビエントリが 1 個残るだけ
    // で、cosmetic な leak。error path と同じ semantics に揃える。
    let _ = shm_unlink(name.as_c_str());

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

/// `/midori-<pid_hex8>-<nanos_hex8>-<counter_hex4>` 形式のユニーク名を
/// 固定幅で組み立てる。
///
/// - `pid_hex8`: `u32` PID を 8 桁の 16 進数で固定幅化（u32 全域を網羅）。
///   異なるプロセス間の衝突を完全に避ける（同 OS 上で同時に走る同 PID は
///   定義上ありえない）。
/// - `nanos_hex8`: `SystemTime::now()` の subsec nanos (≤ 10^9 ≈ 2^30) を
///   8 桁の 16 進数で固定幅化。プロセス内 / 異プロセスの時間方向で衝突確率
///   を下げる。
/// - `counter_hex4`: 同一プロセス内モノトニックカウンタを 16 進 4 桁
///   (`% 0x1_0000`) で固定幅化。同一 nanos で連続生成された場合の決定的
///   な区別をつける。
///
/// 全長: `/midori-`(8) + 8 + `-`(1) + 8 + `-`(1) + 4 = 30 byte。これは
/// macOS の `PSHMNAMLEN = 31` 上限内に **入力 PID / nanos / counter の値
/// に依らず常に** 収まる（10 進 format と異なり可変長になることがない）。
fn build_shm_name(pid: u32) -> String {
    let counter = SHM_NAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    // 16 進固定幅: pid は u32 全域 8 桁、nanos も 8 桁 (subsec nanos は
    // 最大 999_999_999 < 0x4000_0000 で 8 桁内に収まる)、counter は 4 桁
    // (16 bit) で wrap させる。これで PSHMNAMLEN 超過の可能性を構造的に
    // 排除できる。
    let name = format!(
        "/midori-{pid:08x}-{:08x}-{:04x}",
        nanos & 0xFFFF_FFFF,
        counter & 0xFFFF,
    );
    debug_assert!(
        name.len() == 30,
        "shm name should be exactly 30 bytes (fixed-width hex): {name} ({} bytes)",
        name.len()
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
    fn it_should_keep_shm_name_within_pshmnamlen_for_max_pid() {
        // 最大 PID (u32::MAX) でも PSHMNAMLEN を超えないことを確認する。
        // 16 進固定幅化によって pid 値に依らず常に 30 byte になる契約を
        // ここで守らせる（構造的保証の回帰テスト）。
        let name = build_shm_name(u32::MAX);
        assert_eq!(
            name.len(),
            30,
            "name must be exactly 30 bytes regardless of pid: {name} ({} bytes)",
            name.len()
        );
        assert!(
            name.len() <= MAX_SHM_NAME_LEN,
            "name too long: {name} ({} bytes)",
            name.len()
        );
        assert!(name.starts_with("/midori-"), "unexpected prefix: {name}");
    }

    #[test]
    fn it_should_keep_shm_name_within_pshmnamlen_for_zero_pid() {
        // 最小 PID (0) でも長さが変動しない (= 固定幅) ことを確認する。
        // decimal だと "0" は 1 桁、u32::MAX は 10 桁になり 9 byte 差が出るが、
        // 16 進固定幅では同一長になる。
        let name = build_shm_name(0);
        assert_eq!(name.len(), 30, "fixed-width: {name} ({} bytes)", name.len());
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
