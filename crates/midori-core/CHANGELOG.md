# Changelog — midori-core

## 0.3.0 — 2026-04-30

### Breaking changes

- **`shm` モジュールを variable-sized inline ring slot に再設計**（設計: `design/17-driver-comm/01-inline-ring.md`）。
  - `RingSlot` 構造体を撤去。スロット内容は固定 8 byte の `SlotHeader` (`occupied: u8` / `_pad: [u8; 3]` / `payload_len: u32`) と raw payload バイト列 (`slot_size - 8` byte) に分離し、stride 計算で raw memory にアクセスする
  - `ShmHeader` を 56 byte に拡張：`slot_size: u32` / `version: u32` / `_pad: [u8; 32]` を追加
  - `PAYLOAD_INLINE_MAX` 定数を撤去。slot ごとの payload 容量は `ShmHeader.slot_size - 8` で決まる
  - `RingSlot::side_offset` / `side_len` / `_pad2` を撤去。side channel 案は不採用（`design/17-driver-comm/00-overview.md` 参照）

### Added

- 定数: `DEFAULT_SLOT_SIZE = 1032` / `HARD_SLOT_SIZE = 65536` / `MIN_SLOT_SIZE = 12` / `SLOT_HEADER_SIZE = 8`
- ABI version: `SHM_LAYOUT_VERSION = 1` / `MIN_SUPPORTED_SHM_VERSION` / `MAX_SUPPORTED_SHM_VERSION`
- helper: `validate_slot_size` / `align_slot_size` / `shm_total_size` / `slot_offset_in_shm`
- error: `SlotSizeError { NotAligned, TooSmall, TooLarge }`
- `const_assert!` で `ShmHeader = 56 byte` / `SlotHeader = 8 byte` をコンパイル時 lock

### Notes

- `midori-sdk` は本変更に追従して `0.2.0` に bump。FFI 戻り値型も `u8` から `int32_t` に変更（`-2` payload too large の表現）
- 実 driver↔Bridge 間の handshake control channel と shm fd 確保は別 Issue（後続 subtask）の責務

## 0.2.0 — 2026-04-27

### Breaking changes

- **`shm::RingSlot` のレイアウトを raw event payload 形式に差し替え**（設計: `design/15-sdk-bindings-api.md`「SPSC スロットレイアウトの変更」）。
  - 削除されたフィールド: `value_tag` / `device_id` / `specifier` / `value_i64` / `value_f64`
  - 追加されたフィールド: `payload_len: u32` / `side_offset: u64` / `side_len: u32` / `payload: [u8; PAYLOAD_INLINE_MAX]`
  - 内部 padding（`_pad`）は サイズ 6 byte → 3 byte に変更し、新たに `_pad2: [u8; 4]` を追加（レイアウト調整用、API として参照される想定なし）
  - 旧スロットは Layer 2 binding 後の post-binding 形（`device_id` + `specifier` + `value`）だったが、新スロットは Driver → Bridge 間で msgpack バイト列を運ぶ raw event 形式となる。binding 適用は Bridge 側の責務へ移動（`design/layers/02-input-recognition/binding-requirements.md` 参照）
- **`shm::value_tag` モジュールを削除**。`BOOL_FALSE` / `BOOL_TRUE` / `PULSE` / `INT` / `FLOAT` / `NULL` の定数も合わせて廃止
- **`shm::DEVICE_ID_MAX` / `shm::SPECIFIER_MAX` を削除**。スロットに device id / specifier フィールドが存在しなくなったため

### Added

- `shm::PAYLOAD_INLINE_MAX` 定数（240 byte）— inline payload の最大サイズ
- `RingSlot::side_offset` / `side_len` フィールド — `payload_len > PAYLOAD_INLINE_MAX` の payload を side channel（mmap プール）に逃すためのポインタ枠。side channel 本体の確保・割り当て・GC は別途実装する（本クレートのスコープ外）

### Notes

- 旧 `RingSlot` を引数に取っていた `midori-sdk` の SPSC FFI（`midori_sdk_spsc_*`）は 新 `RingSlot` レイアウトに追従し、C ヘッダ（`midori_sdk.h`）も再生成される
- side channel が未実装の段階では `payload_len > PAYLOAD_INLINE_MAX` の emit はサポートされず、driver は inline 範囲（240 byte）内に収まる payload のみ送出する運用とする

## 0.1.0 — 2026-04-23

初版。`Value` / `ValueType` / `ValueRange` / `OutOfRange` / `SignalSpecifier` / `ComponentState` / `Signal` / `IpcEvent` / 旧 `RingSlot`（post-binding 形）/ `ShmHeader` を提供。
