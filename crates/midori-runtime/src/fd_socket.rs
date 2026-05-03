//! UNIX domain socket 越しに file descriptor を 1 個だけ受け渡しする
//! 薄いヘルパー。
//!
//! Bridge は handshake で確保した shm fd を driver subprocess に渡す必要
//! がある。pipe では fd を運べないため、`SCM_RIGHTS` を載せた `sendmsg(2)`
//! / `recvmsg(2)` を使う。本 module は `nix::sys::socket` の薄いラッパーで:
//!
//! - 送信側 [`send_fd`]: 1 byte の dummy payload と一緒に `OwnedFd` 1 個を
//!   `SCM_RIGHTS` 制御メッセージとして送る
//! - 受信側 [`recv_fd`]: 1 byte の dummy payload と `SCM_RIGHTS` を 1 個
//!   読み出し、`OwnedFd` を返す
//!
//! payload の中身は本 module では使わない（fd 1 個だけが意味を持つ）。
//! caller が独自の handshake バイト（例: ack 種別）を載せたい場合の拡張は
//! 別 API で行う。
//!
//! 設計参照: `design/15-sdk-bindings-api.md` Phase 1 / L1-2「Bridge との
//! fd 受け渡しプロトコル」、`design/17-driver-comm/01-inline-ring.md`
//! 「Handshake プロトコル」step 5。

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;

use nix::sys::socket::{recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags};

/// 1 byte のセンチネル payload。`sendmsg`/`recvmsg` は payload 0 byte を
/// 受け付けない実装が多いため、最小サイズの 1 byte を載せる。値そのものは
/// 利用せず、byte が届いた事実だけを使う。
const HANDSHAKE_BYTE: u8 = b'\x01';

/// `stream` 越しに `fd` を 1 個送る。
///
/// 内部で 1 byte の dummy payload と一緒に `SCM_RIGHTS(fd)` を 1 個だけ
/// 載せた `sendmsg(2)` を発行する。fd の所有権は kernel 側にコピーされる
/// （送信元プロセスでは引き続き有効、受信プロセスは duplicate された fd を
/// 受け取る）。caller は呼出後に自前の `BorrowedFd` を `drop` してよい。
///
/// # Errors
///
/// `sendmsg(2)` が失敗した場合（pipe broken、shm fd 無効、等）に `Err` を返す。
pub fn send_fd(stream: &UnixStream, fd: BorrowedFd<'_>) -> io::Result<()> {
    use std::io::IoSlice;

    let bytes = [HANDSHAKE_BYTE];
    let iov = [IoSlice::new(&bytes)];
    let raw = [fd.as_raw_fd()];
    let cmsgs = [ControlMessage::ScmRights(&raw)];

    sendmsg::<()>(stream.as_raw_fd(), &iov, &cmsgs, MsgFlags::empty(), None)
        .map_err(io::Error::from)?;
    Ok(())
}

/// `stream` から fd を 1 個受け取る。
///
/// 受信した `SCM_RIGHTS` の中身を [`OwnedFd`] として返す。dummy payload
/// バイトは捨てる。
///
/// # Errors
///
/// 以下のいずれか:
///
/// - `recvmsg(2)` が失敗（peer が socket を閉じた、等）
/// - 受信メッセージに `SCM_RIGHTS` 制御メッセージが無い
/// - `SCM_RIGHTS` に fd が 0 個 or 2 個以上含まれていた
pub fn recv_fd(stream: &UnixStream) -> io::Result<OwnedFd> {
    use std::io::IoSliceMut;

    let mut byte_buf = [0u8; 1];
    let mut iov = [IoSliceMut::new(&mut byte_buf)];
    // SCM_RIGHTS 1 個分のスペース（fd 1 個 = `RawFd` 4 byte + cmsghdr 分）。
    // nix が `cmsg_space!` マクロを提供しているのでそれを使って正確な
    // バッファサイズを取る。
    let mut cmsg_buf = nix::cmsg_space!([std::os::fd::RawFd; 1]);

    let msg = recvmsg::<()>(
        stream.as_raw_fd(),
        &mut iov,
        Some(&mut cmsg_buf),
        MsgFlags::empty(),
    )
    .map_err(io::Error::from)?;

    let mut received: Option<OwnedFd> = None;
    for cmsg in msg.cmsgs().map_err(io::Error::from)? {
        if let ControlMessageOwned::ScmRights(fds) = cmsg {
            // 「ちょうど 1 個」を要求する。0 個なら peer が SCM_RIGHTS なし
            // で送ってきた、2 個以上なら本 helper のコントラクト違反。
            if fds.len() != 1 {
                // 受け取った fd をリークしないよう即 close する。
                #[allow(unsafe_code)]
                for raw in fds {
                    // SAFETY: `recvmsg` が `fds` に格納した値はちょうど今
                    // 受信したばかりの所有 fd。重複所有は無く、`OwnedFd`
                    // で包んで即 drop すれば close される。
                    let _ = unsafe { OwnedFd::from_raw_fd(raw) };
                }
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "fd_socket: expected exactly 1 fd in SCM_RIGHTS",
                ));
            }
            // SAFETY: 同上。`recvmsg` から受け取った所有 fd を OwnedFd に
            // 移し替えるだけで、Drop で適切に close される。
            #[allow(unsafe_code)]
            let owned = unsafe { OwnedFd::from_raw_fd(fds[0]) };
            received = Some(owned);
        }
    }

    received.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "fd_socket: no SCM_RIGHTS control message received",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::thread;

    #[test]
    fn it_should_round_trip_a_pipe_fd_between_two_unix_stream_endpoints() {
        // socketpair で繋いだ 2 endpoint を、別スレッドで送受信する。
        let (sender, receiver) = UnixStream::pair().expect("UnixStream::pair must succeed");

        // 送る fd: pipe の read 側を作って渡す。受信側で同じ pipe を read
        // できれば「同じ kernel オブジェクトを共有している」確認になる。
        let (pipe_r, mut pipe_w) = os_pipe::pipe().expect("os_pipe must succeed");

        let join = thread::spawn(move || {
            send_fd(&sender, pipe_r.as_fd()).expect("send_fd");
            // sender 側は元の pipe_r を drop。fd は kernel 内で複製済み。
            drop(pipe_r);
        });

        let received_fd = recv_fd(&receiver).expect("recv_fd");
        join.join().expect("sender thread");

        // 送信側だけが pipe_w を持っているので write → 受信した fd で read。
        pipe_w.write_all(b"X").expect("write pipe");
        drop(pipe_w);

        let mut got = [0u8; 1];
        // `OwnedFd` から `File` への移行は `From<OwnedFd>` が安全に提供する
        // ので unsafe 不要。fd の所有権がそのまま File に移る。
        let mut as_file = std::fs::File::from(received_fd);
        as_file.read_exact(&mut got).expect("read pipe");
        assert_eq!(got, [b'X']);
    }

    #[test]
    fn it_should_fail_when_peer_drops_socket_without_sending() {
        let (sender, receiver) = UnixStream::pair().expect("UnixStream::pair must succeed");
        // sender を drop すると receiver の recvmsg は EOF を返す（recvmsg
        // 自体は Ok だが iov に 0 byte、cmsgs も空）。本 helper は SCM_RIGHTS
        // 不在を InvalidData として拒否する。
        drop(sender);
        let err = recv_fd(&receiver).expect_err("peer dropped");
        assert!(matches!(err.kind(), io::ErrorKind::InvalidData));
    }
}
