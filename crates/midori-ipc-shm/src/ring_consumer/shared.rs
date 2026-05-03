//! Linux / macOS 共通の `mmap` ヘルパー。
//!
//! shm 確保経路は OS ごとに分かれるが (`memfd_create` vs `shm_open`)、
//! 確保した fd を `mmap` で書き込み可能 / shared にマップする部分は
//! 完全に共通。`unsafe` を集中させるため、本 module 1 箇所に閉じ込める。

use std::os::fd::OwnedFd;

use memmap2::{MmapMut, MmapOptions};

use super::CreateError;

/// `fd` から `len` byte を `MmapOptions` で `map_mut` する。失敗時は
/// `CreateError::Os { operation: "mmap", source }` に包んで返す。
///
/// `fd` は呼出側が確保した直後の owned な shm fd を借用で受け取る。
/// memmap2 の `map_mut` は `MmapAsRawDesc` (= `AsRawFd`) を要求するが、
/// `&OwnedFd` がこの bound を満たすので追加 trait は不要。
pub(super) fn map_shared_fd(
    fd: &OwnedFd,
    len: usize,
    mut options: MmapOptions,
) -> Result<MmapMut, CreateError> {
    // SAFETY: `fd` は caller が直前に shm 確保 syscall (`memfd_create` /
    // `shm_open`) で得たばかりの fresh な fd。同一プロセス内で他 mapping は
    // 存在しない。`len` は caller が ftruncate で設定した値と一致する。
    // memmap2 の `map_mut` の安全性要件は「他者が mmap 領域を書き換えない、
    // または書き換えに対する整合性を caller が担保する」だが、本 ring layout
    // 自体が「driver から concurrent に書かれることを Acquire/Release で
    // 同期する」前提で設計されている (詳細: ring layout の memory ordering
    // は `midori_core::shm` の `ShmHeader` doc 参照)。残る要件は overflow /
    // null / alignment で、それらは memmap2 内部で検査される。
    #[allow(unsafe_code)]
    let mmap = unsafe {
        options
            .len(len)
            .map_mut(fd)
            .map_err(|source| CreateError::Os {
                operation: "mmap",
                source,
            })?
    };
    Ok(mmap)
}
