# Changelog — midori-core

## 0.2.0 — 2026-04-27

### Breaking changes

- **`shm::RingSlot` のレイアウトを raw event payload 形式に差し替え**（MEW-41 / `design/15-sdk-bindings-api.md`）。
  - 削除されたフィールド: `value_tag` / `device_id` / `specifier` / `value_i64` / `value_f64`
  - 追加されたフィールド: `payload_len: u32` / `side_offset: u64` / `side_len: u32` / `payload: [u8; PAYLOAD_INLINE_MAX]`
  - 内部 padding（`_pad`）は サイズ 6 byte → 3 byte に変更し、新たに `_pad2: [u8; 4]` を追加（レイアウト調整用、API として参照される想定なし）
  - 旧スロットは Layer 2 binding 後の post-binding 形（`device_id` + `specifier` + `value`）だったが、新スロットは Driver → Bridge 間で msgpack バイト列を運ぶ raw event 形式となる。binding 適用は Bridge 側の責務へ移動（`design/layers/02-input-recognition/binding-requirements.md` 参照）
- **`shm::value_tag` モジュールを削除**。`BOOL_FALSE` / `BOOL_TRUE` / `PULSE` / `INT` / `FLOAT` / `NULL` の定数も合わせて廃止
- **`shm::DEVICE_ID_MAX` / `shm::SPECIFIER_MAX` を削除**。スロットに device id / specifier フィールドが存在しなくなったため

### Added

- `shm::PAYLOAD_INLINE_MAX` 定数（240 byte）— inline payload の最大サイズ
- `RingSlot::side_offset` / `side_len` フィールド — `payload_len > PAYLOAD_INLINE_MAX` の payload を side channel（mmap プール）に逃すためのポインタ枠。side channel 本体の確保・割り当て・GC は別 Issue（MEW-43）で実装

### Notes

- 旧 `RingSlot` を引数に取っていた `midori-sdk` の SPSC FFI（`midori_sdk_spsc_*`）は 新 `RingSlot` レイアウトに追従し、C ヘッダ（`midori_sdk.h`）も再生成される
- side channel が未実装の段階では `payload_len > PAYLOAD_INLINE_MAX` の emit はサポートされず、driver は inline 範囲（240 byte）内に収まる payload のみ送出する運用とする

## 0.1.0 — 2026-04-23

初版。`Value` / `ValueType` / `ValueRange` / `OutOfRange` / `SignalSpecifier` / `ComponentState` / `Signal` / `IpcEvent` / 旧 `RingSlot`（post-binding 形）/ `ShmHeader` を提供。
