//! Linux 向け shm 確保経路。`memfd_create(2)` + `ftruncate(2)` + `mmap(2)`。
//!
//! `memfd_create` は anonymous shm を返す Linux 固有 syscall。返却 fd は
//! ファイルシステム上の名前を持たないため、衝突回避のための名前生成 /
//! `unlink` 相当の操作は不要。Bridge と driver は `SCM_RIGHTS` で fd を共有
//! して同 kernel 領域を attach する。
//!
//! macOS 経路 (`super::macos`) との API 境界は `create_shm_for_ring` の
//! シグネチャで揃えている: `(MmapMut, OwnedFd)` を返し、上位 dispatch
//! (`super::mod`) で `RingConsumerCore` に包む。

use std::ffi::CString;
use std::os::fd::OwnedFd;

use memmap2::{MmapMut, MmapOptions};

use super::{shared::map_shared_fd, CreateError};

/// `slot_size` から `shm_bytes` 分の anonymous shm を確保し、`(mmap, fd)` を
/// 返す。返される `OwnedFd` は driver subprocess に `SCM_RIGHTS` で渡す前提。
pub(super) fn create_shm_for_ring(shm_bytes: usize) -> Result<(MmapMut, OwnedFd), CreateError> {
    let owned_fd = create_memfd().map_err(|source| CreateError::Os {
        operation: "memfd_create",
        source,
    })?;

    // ftruncate でファイルを目標サイズに拡張する。memfd は初期サイズ 0 の
    // 仮想ファイルなので、mmap する前に必ず必要。
    let truncate_len = i64::try_from(shm_bytes).map_err(|_| CreateError::Os {
        operation: "ftruncate (size_t→i64 cast)",
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "shm size exceeds i64::MAX",
        ),
    })?;
    nix::unistd::ftruncate(&owned_fd, truncate_len).map_err(|errno| CreateError::Os {
        operation: "ftruncate",
        source: std::io::Error::from(errno),
    })?;

    let mmap = map_shared_fd(&owned_fd, shm_bytes, MmapOptions::new())?;
    Ok((mmap, owned_fd))
}

/// `memfd_create(2)` の薄いラッパー。close-on-exec を立てる。
fn create_memfd() -> std::io::Result<OwnedFd> {
    use nix::sys::memfd::{memfd_create, MFdFlags};
    let name = CString::new("midori-ring").expect("static C string is non-NUL");
    memfd_create(name.as_c_str(), MFdFlags::MFD_CLOEXEC).map_err(std::io::Error::from)
}
