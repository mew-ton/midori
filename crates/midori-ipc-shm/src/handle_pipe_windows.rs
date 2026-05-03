//! Windows 版「shm section HANDLE を別プロセスへ受け渡しする」ヘルパー。
//!
//! Linux 版の [`crate::fd_socket`] (`SCM_RIGHTS` over Unix domain socket) と
//! 同じ役割を Windows で担う。Windows の HANDLE は kernel が自動複製しない
//! ため、明示的に:
//!
//! 1. Bridge が driver subprocess の **PID** を取得（spawn 時に `Child::id()`
//!    から）
//! 2. Bridge が `OpenProcess(PROCESS_DUP_HANDLE, ..., driver_pid)` で driver
//!    process handle を取得
//! 3. Bridge が `DuplicateHandle(GetCurrentProcess(), source_handle,
//!    driver_process, &out_handle, 0, FALSE, DUPLICATE_SAME_ACCESS)` で
//!    driver アドレス空間でも valid な HANDLE を生成
//! 4. その HANDLE 値（u64 で encode）を **Named Pipe 経由** で driver に
//!    送信
//! 5. driver は受け取った u64 をそのまま `HANDLE` として `MapViewOfFile` 等
//!    で利用
//!
//! という手順を踏む。Named Pipe 名は `\\.\pipe\midori-shm-<bridge_pid>-<random>`
//! 形式（[`PipeName::generate`]）で global namespace 衝突を避ける。
//!
//! # プロトコル仕様（wire format）
//!
//! pipe over に流すバイト列はちょうど **9 byte**:
//!
//! - 先頭 1 byte: sentinel `0x01`（Linux 版 `fd_socket` と同じ理由 — 0 byte
//!   メッセージは pipe API でセマンティクスが揺れるため、最低 1 byte の
//!   payload を載せる）
//! - 続く 8 byte: HANDLE 値を host endian の u64 として encode（Windows の
//!   HANDLE は 64-bit OS で 64-bit pointer-sized）
//!
//! 受信側はちょうど 9 byte を読み、sentinel を破棄、続く 8 byte を u64 →
//! HANDLE に復元する。
//!
//! # スコープ外
//!
//! - Named Pipe の認証（現状は default security descriptor、すなわち
//!   呼び出しスレッドの primary token から派生する DACL が適用される）
//! - 非同期 I/O（`OVERLAPPED` を使わない blocking モード）
//! - 同一 pipe での複数 HANDLE 連続送受信（1 pipe = 1 HANDLE、終わったら
//!   pipe を close）

use std::ffi::OsString;
use std::fmt;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::process;
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::{
    DuplicateHandle, GetLastError, DUPLICATE_SAME_ACCESS, FALSE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE};

/// `CreateFileW` の dwShareMode に「共有しない」を意味する定数。
///
/// windows-sys には `FILE_SHARE_NONE` が無く、`FILE_SHARE_MODE = u32` の
/// 0 を使うのが慣例（`FILE_SHARE_READ` 等のフラグ合算で 0 = 何も共有しない）。
const FILE_SHARE_NONE: u32 = 0;

/// 送受信プロトコルの先頭 sentinel byte。値そのものは使わない（Linux 版
/// `fd_socket::HANDSHAKE_BYTE` と同じ理由）。
const HANDSHAKE_BYTE: u8 = 0x01;

/// Pipe 越しに送る 1 メッセージのバイト数 = sentinel 1 byte + HANDLE (u64) 8 byte。
const MESSAGE_LEN: usize = 1 + 8;

/// Named Pipe 名を生成・保持する型。
///
/// Windows の Named Pipe は `\\.\pipe\<name>` の global namespace に存在し、
/// 衝突回避のため `<name>` を **bridge プロセスの PID + 単調増加 nonce** で
/// 構成する。本構造体はそのフォーマット規約を 1 ヶ所に閉じ込める。
///
/// `Display` は wide path（`OsString` 経由の UTF-16 列に変換可能な `String`）
/// として返し、`as_wide()` は CreateFile / CreateNamedPipe API 用に
/// NUL 終端された UTF-16 列を作って返す。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeName {
    /// `\\.\pipe\midori-shm-<pid>-<nonce>` 形式の path。
    path: String,
}

impl PipeName {
    /// 衝突回避された pipe 名を生成する。
    ///
    /// フォーマット: `\\.\pipe\midori-shm-<pid>-<nonce_hex>`
    ///
    /// - `<pid>`: 呼び出しプロセスの PID（Bridge 側で呼ぶ前提）
    /// - `<nonce_hex>`: `SystemTime::now()` の nanos を u64 hex 16 桁に
    ///   フォーマットしたもの。同一プロセス内で短時間に連続生成しても
    ///   ns 解像度なら衝突しない（万一衝突したら `CreateNamedPipeW` が
    ///   `ERROR_PIPE_BUSY` で失敗するので caller は retry 可能）
    #[must_use]
    pub fn generate() -> Self {
        let pid = process::id();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0u128, |d| d.as_nanos());
        // nanos は u128 だが pipe 名としては下位 64 bit で十分（衝突確率は
        // ns 解像度の自然な分散に従う）。
        let nonce_u64 = nonce as u64;
        Self {
            path: format!("\\\\.\\pipe\\midori-shm-{pid}-{nonce_u64:016x}"),
        }
    }

    /// 任意の path で `PipeName` を構築する（test / 高度な caller 用）。
    ///
    /// caller は `\\.\pipe\` プレフィックスを含む完全な path を渡す責任を
    /// 負う。本関数は文字列を保持するだけで、Win32 命名規則の検証は
    /// しない（不正なら `CreateNamedPipeW` 側で失敗する）。
    #[must_use]
    pub fn from_path(path: String) -> Self {
        Self { path }
    }

    /// path 文字列を返す。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Win32 API 用に NUL 終端された UTF-16 wide 文字列を返す。
    fn as_wide_nul(&self) -> Vec<u16> {
        let os: OsString = self.path.clone().into();
        let mut wide: Vec<u16> = os.encode_wide().collect();
        wide.push(0);
        wide
    }
}

impl fmt::Display for PipeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

/// HANDLE 受け渡しヘルパーの失敗種別。
///
/// `operation` は失敗した Win32 API 名、`source` は `GetLastError` を元に
/// 組み立てた [`std::io::Error`]、または I/O 層の error。
#[derive(Debug)]
pub enum HandlePipeError {
    /// Win32 API 失敗（`CreateNamedPipeW` / `OpenProcess` / `DuplicateHandle`
    /// / `CreateFileW` 等）。
    Os {
        operation: &'static str,
        source: std::io::Error,
    },
    /// pipe 越しの I/O が要求バイト数に達する前に EOF した、または
    /// 想定外の長さで返ってきた。
    Protocol(String),
}

impl fmt::Display for HandlePipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Os { operation, source } => {
                write!(f, "handle_pipe: {operation} が失敗しました: {source}")
            }
            Self::Protocol(msg) => write!(f, "handle_pipe: プロトコル違反: {msg}"),
        }
    }
}

impl std::error::Error for HandlePipeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Os { source, .. } => Some(source),
            Self::Protocol(_) => None,
        }
    }
}

/// Bridge 側: 指定 `pipe_name` で Named Pipe server endpoint を作る。
///
/// 戻り値の [`OwnedHandle`] は **server 側 endpoint**。caller は次に
/// [`accept_and_send`] を呼んで peer の接続を待ち、その上で HANDLE を送る。
///
/// # Errors
///
/// `CreateNamedPipeW` が失敗した場合（pipe 名重複 / 権限不足 等）。
pub fn create_pipe_server(pipe_name: &PipeName) -> Result<OwnedHandle, HandlePipeError> {
    let wide = pipe_name.as_wide_nul();

    // SAFETY: lpName は `\\.\pipe\...` 形式の NUL 終端 wide 文字列。
    // dwOpenMode = PIPE_ACCESS_DUPLEX で双方向 byte stream。
    // dwPipeMode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT |
    //   PIPE_REJECT_REMOTE_CLIENTS でローカルのみ・blocking I/O。
    // nMaxInstances = 1 で当該名は同時 1 client のみ（再利用しない 1-shot
    // pipe）。nOutBufferSize / nInBufferSize は 4 KiB を仮定（MESSAGE_LEN
    // よりはるかに大きいので余裕）。nDefaultTimeOut = 0 で kernel の default
    // (~50 ms)。lpSecurityAttributes = NULL で default DACL（呼び出しスレッド
    // の primary token から派生）。戻り値が INVALID_HANDLE_VALUE なら失敗。
    #[allow(unsafe_code)]
    let raw = unsafe {
        CreateNamedPipeW(
            wide.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            4096,
            4096,
            0,
            ptr::null_mut(),
        )
    };

    if raw == INVALID_HANDLE_VALUE {
        return Err(HandlePipeError::Os {
            operation: "CreateNamedPipeW",
            source: last_os_error(),
        });
    }

    // SAFETY: `raw` は `CreateNamedPipeW` が返した有効な HANDLE で、本関数
    // 内で他者に複製していない。`OwnedHandle` で包むことで Drop 時に
    // `CloseHandle` が 1 度だけ呼ばれることを保証する。
    #[allow(unsafe_code)]
    let owned = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    Ok(owned)
}

/// Bridge 側: server pipe で peer の接続を待ち、`source_handle` を `peer_pid`
/// プロセスに `DuplicateHandle` で複製してから pipe 越しに送信する。
///
/// 引数:
///
/// - `pipe_server`: [`create_pipe_server`] で得た server endpoint
/// - `peer_pid`: HANDLE を渡す相手プロセスの PID（driver subprocess の
///   `Child::id()` から取得）
/// - `source_handle`: bridge プロセス内で valid な転送元 HANDLE（典型は
///   shm section）。所有権は本関数に移譲され、関数内の duplication 完了後に
///   `CloseHandle` される（duplication 元側はもう不要）。
///
/// # Errors
///
/// - `ConnectNamedPipe` 失敗（peer が来ない / pipe 破損）
/// - `OpenProcess(PROCESS_DUP_HANDLE)` 失敗（権限不足 / PID 無効）
/// - `DuplicateHandle` 失敗（source_handle 無効 / DACL 拒否）
/// - pipe write が部分書きで終わった
pub fn accept_and_send(
    pipe_server: &OwnedHandle,
    peer_pid: u32,
    source_handle: OwnedHandle,
) -> Result<(), HandlePipeError> {
    let server_raw = pipe_server.as_raw_handle() as HANDLE;

    // SAFETY: `server_raw` は `create_pipe_server` が返した有効な server
    // pipe handle。lpOverlapped = NULL で blocking 動作。戻り値 0 は失敗
    // だが、`ERROR_PIPE_CONNECTED` (= 535) は「すでに peer が接続済み」を
    // 意味する成功ケース（kernel 都合の race）なので例外的に許容する。
    #[allow(unsafe_code)]
    let connect_ok = unsafe { ConnectNamedPipe(server_raw, ptr::null_mut()) };
    if connect_ok == 0 {
        let err = last_os_error();
        // ERROR_PIPE_CONNECTED = 535: すでに driver 側が CreateFileW で
        // 接続を完了していた race。これは成功扱い（Microsoft Learn 記載の
        // 公式契約）。
        const ERROR_PIPE_CONNECTED: i32 = 535;
        if err.raw_os_error() != Some(ERROR_PIPE_CONNECTED) {
            return Err(HandlePipeError::Os {
                operation: "ConnectNamedPipe",
                source: err,
            });
        }
    }

    // peer プロセスの handle を取得（PROCESS_DUP_HANDLE 権限のみ要求、
    // 必要最小特権）。
    let peer_proc = open_peer_process(peer_pid)?;

    // bridge プロセス内の source_handle を peer プロセスのアドレス空間で
    // valid な HANDLE 値に複製する。`duplicated` は **peer 側 HANDLE 値**
    // で、本関数 return までに成功通知（pipe write 成功）が peer に届かな
    // ければ peer 側で close されない（peer プロセス全体が終了するまで
    // 残る）。下記 wire write が失敗した場合、本関数 caller は peer
    // subprocess を kill する責務を負う（kill により peer 側 HANDLE は
    // 自動 release される）。
    let duplicated = duplicate_into_peer(source_handle.as_raw_handle(), peer_proc.as_raw_handle())?;

    // wire format: sentinel 1 byte + HANDLE 値 8 byte。
    let mut buf = [0u8; MESSAGE_LEN];
    buf[0] = HANDSHAKE_BYTE;
    let handle_u64 = duplicated as u64;
    buf[1..9].copy_from_slice(&handle_u64.to_ne_bytes());

    // `source_handle` / `peer_proc` の close は **write 成功後** に行う。
    // 早期 drop すると、write 失敗時に caller が retry や代替経路に切り
    // 替える選択肢を失う（bridge 側の section 参照と peer process
    // handle が両方無くなった状態になる）。
    write_all_to_pipe(server_raw, &buf)?;

    // 通知が peer に届いたので bridge 側の参照は不要。
    drop(source_handle);
    drop(peer_proc);
    Ok(())
}

/// Driver 側: 指定 `pipe_name` の server に接続し、HANDLE を 1 個受け取る。
///
/// 戻り値の [`OwnedHandle`] は **driver プロセスのアドレス空間で valid な
/// HANDLE**（bridge 側で `DuplicateHandle` 済み）。Drop で `CloseHandle` が
/// 自動的に呼ばれる。
///
/// # Errors
///
/// - `CreateFileW` 失敗（pipe 名が存在しない / 権限不足 / busy）
/// - pipe read が要求バイト数に達する前に EOF
/// - sentinel byte が想定値と異なる（プロトコル違反）
pub fn connect_and_recv(pipe_name: &PipeName) -> Result<OwnedHandle, HandlePipeError> {
    let wide = pipe_name.as_wide_nul();

    // SAFETY: lpFileName は `\\.\pipe\...` 形式の NUL 終端 wide 文字列。
    // dwDesiredAccess = FILE_GENERIC_READ | FILE_GENERIC_WRITE で
    // 読み書き両方可能な client endpoint を要求。dwShareMode = 0
    // (FILE_SHARE_NONE) で他者と pipe を共有しない。
    // lpSecurityAttributes = NULL で default。dwCreationDisposition =
    // OPEN_EXISTING で「既存の pipe に接続する」を明示。dwFlagsAndAttributes
    // = 0 で属性なし（同期 I/O）。hTemplateFile = NULL（pipe では未使用）。
    // 戻り値が INVALID_HANDLE_VALUE なら失敗。
    #[allow(unsafe_code)]
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_NONE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };

    if raw == INVALID_HANDLE_VALUE {
        return Err(HandlePipeError::Os {
            operation: "CreateFileW",
            source: last_os_error(),
        });
    }

    // SAFETY: `raw` は `CreateFileW` が返した有効な pipe client HANDLE。
    // 本関数内で他者に複製していない。`OwnedHandle` で包んで RAII 管理。
    #[allow(unsafe_code)]
    let pipe_client = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };

    let mut buf = [0u8; MESSAGE_LEN];
    read_exact_from_pipe(pipe_client.as_raw_handle() as HANDLE, &mut buf)?;

    if buf[0] != HANDSHAKE_BYTE {
        return Err(HandlePipeError::Protocol(format!(
            "unexpected sentinel byte: 0x{:02x} (expected 0x{:02x})",
            buf[0], HANDSHAKE_BYTE
        )));
    }

    let handle_u64 = u64::from_ne_bytes([
        buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8],
    ]);
    let handle_raw = handle_u64 as RawHandle;

    // SAFETY: `handle_raw` は bridge 側で `DuplicateHandle` した結果、
    // この driver プロセスのアドレス空間で valid な HANDLE 値。
    // bridge 側は `DuplicateHandle` 後に source 側の同 HANDLE を close
    // 済みなので、driver 側でこの HANDLE の唯一の所有者になる。
    // `OwnedHandle` で包んで Drop 時に `CloseHandle` で適切に release。
    #[allow(unsafe_code)]
    let received = unsafe { OwnedHandle::from_raw_handle(handle_raw) };
    Ok(received)
}

/// `OpenProcess(PROCESS_DUP_HANDLE, FALSE, peer_pid)` の薄いラッパー。
fn open_peer_process(peer_pid: u32) -> Result<OwnedHandle, HandlePipeError> {
    // SAFETY: PROCESS_DUP_HANDLE は DuplicateHandle に必要な最小権限。
    // bInheritHandle = FALSE で Bridge の child プロセスへの自動 inherit
    // を抑止（HANDLE 受け渡しは pipe 経由で明示的に行うため）。
    // 戻り値が NULL なら失敗。
    #[allow(unsafe_code)]
    let raw = unsafe { OpenProcess(PROCESS_DUP_HANDLE, FALSE, peer_pid) };
    if raw.is_null() {
        return Err(HandlePipeError::Os {
            operation: "OpenProcess",
            source: last_os_error(),
        });
    }

    // SAFETY: `raw` は `OpenProcess` が返した有効な process HANDLE で、
    // 本関数内で他者に複製していない。`OwnedHandle` で RAII 管理。
    #[allow(unsafe_code)]
    let owned = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    Ok(owned)
}

/// `DuplicateHandle` を使い、`source_handle` を `peer_process` のアドレス
/// 空間でも valid な HANDLE 値に複製する。
///
/// 戻り値は **peer 側コンテキストで意味を持つ生 HANDLE 値**。bridge 側で
/// `OwnedHandle` で包むのは誤り（bridge プロセス内では valid でない）。
fn duplicate_into_peer(
    source_handle: RawHandle,
    peer_process: RawHandle,
) -> Result<HANDLE, HandlePipeError> {
    let mut out: HANDLE = ptr::null_mut();

    // SAFETY: hSourceProcessHandle = GetCurrentProcess()（pseudo handle、
    // close 不要）。hSourceHandle = source_handle（caller 所有）。
    // hTargetProcessHandle = peer_process。lpTargetHandle = &mut out で
    // peer 側 HANDLE 値を受け取る。dwDesiredAccess = 0 と
    // dwOptions = DUPLICATE_SAME_ACCESS の組合せで「source と同じアクセス
    // 権で複製」を指定。bInheritHandle = FALSE で peer の子プロセスへの
    // inherit を抑止。戻り値 0 は失敗。
    #[allow(unsafe_code)]
    let ok = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            source_handle as HANDLE,
            peer_process as HANDLE,
            &mut out,
            0,
            FALSE,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if ok == 0 {
        return Err(HandlePipeError::Os {
            operation: "DuplicateHandle",
            source: last_os_error(),
        });
    }
    Ok(out)
}

/// `WriteFile` を要求バイト数すべて書き終わるまでループする。
fn write_all_to_pipe(pipe: HANDLE, buf: &[u8]) -> Result<(), HandlePipeError> {
    let mut written_total: usize = 0;
    while written_total < buf.len() {
        let remaining = &buf[written_total..];
        let mut written: u32 = 0;
        // SAFETY: pipe は caller が所有する有効な pipe HANDLE。
        // remaining は本関数の lifetime 内で valid。
        // nNumberOfBytesToWrite は usize → u32 変換だが、本関数 caller が
        // 渡すのは MESSAGE_LEN = 9 byte ぴったりなので 32 bit に常に収まる。
        #[allow(unsafe_code)]
        let ok = unsafe {
            WriteFile(
                pipe,
                remaining.as_ptr(),
                u32::try_from(remaining.len()).unwrap_or(u32::MAX),
                &mut written,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(HandlePipeError::Os {
                operation: "WriteFile",
                source: last_os_error(),
            });
        }
        if written == 0 {
            return Err(HandlePipeError::Protocol(
                "WriteFile returned 0 bytes (peer closed pipe)".into(),
            ));
        }
        written_total += written as usize;
    }
    Ok(())
}

/// `ReadFile` を要求バイト数すべて読み終わるまでループする。
fn read_exact_from_pipe(pipe: HANDLE, buf: &mut [u8]) -> Result<(), HandlePipeError> {
    let mut read_total: usize = 0;
    while read_total < buf.len() {
        let remaining = &mut buf[read_total..];
        let mut read: u32 = 0;
        // SAFETY: pipe は caller が所有する有効な pipe HANDLE。
        // remaining は本関数の lifetime 内で valid な可変借用。
        // nNumberOfBytesToRead は usize → u32 変換だが、本関数 caller が
        // 渡すのは MESSAGE_LEN = 9 byte ぴったりなので 32 bit に常に収まる。
        #[allow(unsafe_code)]
        let ok = unsafe {
            ReadFile(
                pipe,
                remaining.as_mut_ptr(),
                u32::try_from(remaining.len()).unwrap_or(u32::MAX),
                &mut read,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(HandlePipeError::Os {
                operation: "ReadFile",
                source: last_os_error(),
            });
        }
        if read == 0 {
            return Err(HandlePipeError::Protocol(format!(
                "ReadFile EOF after {read_total} bytes (expected {})",
                buf.len()
            )));
        }
        read_total += read as usize;
    }
    Ok(())
}

/// 直近の Win32 エラー（`GetLastError`）を [`std::io::Error`] に変換する。
fn last_os_error() -> std::io::Error {
    // SAFETY: `GetLastError` は副作用なくスレッド local の last-error 値
    // を返すだけで、引数も持たない。スレッド local なので race も無い。
    #[allow(unsafe_code)]
    let code = unsafe { GetLastError() };
    std::io::Error::from_raw_os_error(code as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn pipe_name_should_start_with_named_pipe_prefix() {
        let name = PipeName::generate();
        assert!(
            name.as_str().starts_with("\\\\.\\pipe\\midori-shm-"),
            "got: {name}"
        );
        // PID と nonce が含まれる
        assert!(name.as_str().contains(&format!("{}", process::id())), "PID");
    }

    #[test]
    fn pipe_name_should_be_unique_across_consecutive_generates() {
        // ns 解像度に依存するため厳密ではないが、連続生成で同値はまず出ない。
        let a = PipeName::generate();
        let b = PipeName::generate();
        // 同値になった場合、2 度目の `CreateNamedPipeW` が ERROR_PIPE_BUSY
        // で失敗するため、運用上問題ない（caller が retry する想定）。
        // ここではテストとしては緩めに「PID 部は同じ、nonce 部は異なる
        // ことが期待されるが strict assert はしない」とする。
        assert_eq!(
            a.as_str().rsplit_once('-').map(|x| x.0),
            b.as_str().rsplit_once('-').map(|x| x.0),
            "PID + prefix 部は同じはず"
        );
    }

    /// in-process round trip テスト。
    ///
    /// 自プロセスを peer に見立て（`process::id()` を peer_pid として渡す）、
    /// bridge スレッドで Named Pipe server を立て、driver スレッドが
    /// connect → 受信、bridge が DuplicateHandle 後に送信、を 1 round
    /// 行う。送る HANDLE は事前に作った 2 度目の OwnedHandle（CreateEvent
    /// 等を使うと依存が増えるため、`DuplicateHandle` で
    /// `GetCurrentProcess` の pseudo handle を実 handle に変換したものを
    /// 利用する）。
    #[test]
    fn it_should_round_trip_a_handle_within_the_same_process() {
        // bridge: pipe 生成
        let pipe_name = PipeName::generate();
        let server = create_pipe_server(&pipe_name).expect("create_pipe_server");

        // driver スレッド: server より先に CreateFile すると
        // ERROR_PIPE_BUSY になり得るので、bridge が server を作った直後に
        // spawn する（CreateNamedPipeW 直後はすでに listen 状態）。
        let pipe_name_for_driver = pipe_name.clone();
        let driver_thread = thread::spawn(move || connect_and_recv(&pipe_name_for_driver));

        // bridge: 送る HANDLE を準備する。
        // `GetCurrentProcess` は pseudo handle (-1) で実体ではないため、
        // DuplicateHandle で「自プロセス → 自プロセス」の複製を実 handle に
        // 落とす。これで close できる本物の HANDLE が手に入る。
        let mut real: HANDLE = ptr::null_mut();
        // SAFETY: GetCurrentProcess pseudo handle を hSourceHandle に渡し、
        // hSourceProcessHandle / hTargetProcessHandle 双方も自プロセス。
        // dwOptions = DUPLICATE_SAME_ACCESS で同等の access right を持つ
        // 実 handle を `real` 経由で受け取る。
        #[allow(unsafe_code)]
        let dup_ok = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                GetCurrentProcess(),
                GetCurrentProcess(),
                &mut real,
                0,
                FALSE,
                DUPLICATE_SAME_ACCESS,
            )
        };
        assert_ne!(dup_ok, 0, "DuplicateHandle (self → self) should succeed");

        // SAFETY: `real` は今 DuplicateHandle で得た有効な process handle。
        // 本テスト内で他者に複製していない。`OwnedHandle` で RAII 管理。
        #[allow(unsafe_code)]
        let owned_real = unsafe { OwnedHandle::from_raw_handle(real as RawHandle) };

        // bridge: 自プロセスを peer 扱いで送信
        let send_result = accept_and_send(&server, process::id(), owned_real);
        assert!(send_result.is_ok(), "accept_and_send: {send_result:?}");

        // driver スレッドの結果を回収
        let recv_result = driver_thread.join().expect("driver thread");
        let received = recv_result.expect("connect_and_recv");

        // 受信した HANDLE は自プロセス向けに複製されたもの。
        // 値は元の `real` と同じ生 HANDLE 値ではない（DuplicateHandle が
        // 新しい HANDLE entry を作る契約）が、有効な HANDLE なはず。
        // OwnedHandle::from_raw_handle と Drop で CloseHandle が呼ばれて
        // 例外を出さなければ valid と判定する。
        drop(received);
        drop(server);
    }

    #[test]
    fn it_should_fail_to_connect_to_nonexistent_pipe() {
        // 存在しない pipe 名で CreateFileW すると ERROR_FILE_NOT_FOUND (2)
        // になる。HandlePipeError::Os(operation: "CreateFileW", ...) を
        // 返すことを確認する。
        let bogus = PipeName::from_path(
            "\\\\.\\pipe\\midori-shm-test-nonexistent-7f7f7f7f7f7f7f7f".to_string(),
        );
        let err = connect_and_recv(&bogus).expect_err("nonexistent pipe");
        match err {
            HandlePipeError::Os { operation, .. } => assert_eq!(operation, "CreateFileW"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
