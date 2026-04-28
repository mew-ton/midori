# Variable-Sized Ring Slot 案（検討中）

> ステータス：**検討中（採否未決定）**
> 最終更新：2026-04-28

本書は oversized event payload を運ぶ方式の **案 B**。**案 A（[side-channel.md](./side-channel.md)）** と比較検討中。両案の比較と採否判断材料は [README.md](./README.md) を参照。

「driver が宣言する最大 payload サイズに合わせて、起動時 (handshake) に **RingSlot のサイズを動的に決める**」案。各スロットは driver lifetime 内で固定サイズだが、driver プロセスごとに異なる。すべての payload は ring slot に inline で収まり、別 mmap 領域（side channel）は不要。

実装本体（FFI 拡張・テスト）は本書のスコープ外。Driver/Bridge 双方が触る API は **スケッチ** までを示し、実装 Issue で詳細化する。

---

## 全体方針

1. driver は events.yaml の `bytes.max_length` 集合を解析し、`max_payload_size = max(全 bytes.max_length) + 固定オーバーヘッド` を **handshake 時に Bridge へ宣言** する
2. Bridge は受け取った `max_payload_size` で **`ShmHeader.slot_size` を確定** し、`RING_CAPACITY × slot_size` の shm を確保
3. ring slot サイズは driver lifetime 内で **固定**。driver 再起動時のみ変えられる
4. 全 payload が **inline**。`RingSlot::side_offset` / `side_len` は **廃止**
5. back-pressure はリング満杯のみ（`emit_event` が `0` を返す）。payload size 超過は handshake 時にハードに弾かれるため、`-2` の使い道は events.yaml 違反の防衛的検出のみ

---

## スコープ

本書で決めること:

- ハンドシェイクで `max_payload_size` を決めるプロトコル
- `ShmHeader` への `slot_size` 追加と stride 計算
- `RingSlot` の固定サイズ前提の撤廃（payload 部が動的長）
- back-pressure 戻り値仕様の簡略化
- ABI / version の取り扱い方針

スコープ外（**Out of Scope**）:

- driver lifetime 中の `slot_size` 変更（driver 再起動でしか変えられない）
- 同一プロセス内 multi-driver（`design/15-sdk-bindings-api.md` の方針通り別プロセス化）
- 圧縮（msgpack バイト列をそのまま運ぶ）
- 実装本体・テストケース具体記述

---

## RingSlot レイアウト

`RingSlot` は **コンパイル時固定の payload 配列を持たない**。代わりに **`ShmHeader.slot_size` で stride を計算した raw memory アクセス** にする。

### スロットの内部レイアウト

```text
slot offset 0:  occupied: u8
slot offset 1:  _pad: [u8; 3]
slot offset 4:  payload_len: u32
slot offset 8:  payload: [u8; slot_size - 8]
```

ヘッダ部分（`occupied: u8` + `_pad: [u8; 3]` + `payload_len: u32` の合計 8 byte）は固定オフセットに置き、payload は **`slot_size - 8` バイト**続く。`slot_size` は handshake で決まり、`ShmHeader.slot_size` に格納される。

> **alignment 要件**: `slot_size` は **4 byte の倍数** であること（`payload_len: u32` の natural alignment を slot N >= 1 でも維持するため）。この保証は **後述「Handshake プロトコル」step 4** で Bridge が `slot_size` を 4 byte 倍数へ切り上げる処理（`slot_size = ((max_payload_size + 8) + 3) & !3`）により担保される。Driver 側で alignment violation を懸念する必要はない。

### 旧 RingSlot との差分

| フィールド | 旧（side channel 案） | 本案 |
|---|---|---|
| `occupied: u8` + `_pad: [u8; 3]` | あり | あり |
| `payload_len: u32` | あり | あり |
| `side_offset: u64` | あり | **廃止** |
| `side_len: u32` | あり | **廃止** |
| `_pad2: [u8; 4]` | あり | **廃止** |
| `payload: [u8; PAYLOAD_INLINE_MAX]` | 固定 240 byte | **動的（`slot_size - 8`）** |
| 1 スロットの総バイト数 | 264 固定 | `slot_size`（handshake で決定） |

`PAYLOAD_INLINE_MAX` 定数も不要となる。

### ShmHeader の拡張

```rust
#[repr(C)]
pub struct ShmHeader {
    pub write_index: AtomicU64,
    pub read_index: AtomicU64,
    pub slot_size: u32,        // バイト単位、handshake で決まる（4 byte 倍数）
    pub version: u32,          // ABI version（本案で導入。初期値 1）
    // 将来フィールド追加余地（append-only minor bump 時に消費）。
    // 32 byte 確保しておくと、ヘッダ前段 24 byte と合わせて総 56 byte となり、
    // 64 byte cache line 1 本以内に収まる（残り 8 byte は将来追加分の余裕）
    pub _pad: [u8; 32],
}

// 採用時はコンパイル時アサーションで lock する（既存 ShmHeader 16 byte
// アサーションを置き換える）:
//   const _: () = assert!(std::mem::size_of::<ShmHeader>() == 56);
//   const _: () = assert!(std::mem::align_of::<ShmHeader>() == 8);
```

総サイズ：`8 + 8 + 4 + 4 + 32 = 56 byte`、alignment は `AtomicU64` の 8 byte。

`slot_size` は driver 起動時に Bridge が書き込み、それ以降は不変。Driver と Bridge は起動時にこの値を読み込んで stride 計算に使う。

### スロットアクセス

```rust
// 擬似コード
fn slot_ptr(base: *mut u8, header: &ShmHeader, idx: u64) -> *mut u8 {
    let stride = header.slot_size as usize;
    let header_size = std::mem::size_of::<ShmHeader>();
    let offset = header_size + (idx as usize % RING_CAPACITY) * stride;
    base.add(offset)
}
```

---

## Handshake プロトコル

driver 起動時、Bridge 側で shm を確保するまでの流れ:

1. **driver 起動** → events.yaml をパース
2. **driver 側で max_payload_size を計算**:
   - 全 events の `bytes.max_length` を収集
   - msgpack encode 後の最大長 + 固定オーバーヘッド（type 文字列・key 名・msgpack タグ等）
   - 結果を `max_payload_size: u32` として保持
3. **driver → Bridge** に `request_ring(max_payload_size)` を送信（control channel 経由）
4. **Bridge 側で受領**:
   - **slot_size の確定**: `slot_size = ((max_payload_size + 8) + 3) & !3`（ヘッダ 8 byte を加算した上で 4 byte 倍数へ切り上げ。`payload_len: u32` の natural alignment を slot 全数で維持するため）
   - **上限チェック**: `slot_size > MAX_SLOT_SIZE` なら reject（実装上限超過）
   - **shm 全体サイズの確定**: `shm_total = sizeof(ShmHeader) + RING_CAPACITY × slot_size`。これを **ページサイズ（4 KiB）で切り上げ** て allocate。具体式は `shm_mmap_size = (shm_total + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)`（`PAGE_SIZE = 4096`。mmap 単位がページなので shm 全体のみページ整列でよく、slot 単位の page-align は不要）
   - `ShmHeader.slot_size` に確定値を書き込み、`ShmHeader.version = 1`（初期版）で初期化
5. **Bridge → driver** に shm fd を返す
6. **driver は fd を mmap し、`ShmHeader.slot_size` を読み込んで stride 計算を確立**

control channel は `design/15-sdk-bindings-api.md` の Phase 1 / L1-2「Bridge との fd 受け渡しプロトコル」と統合する。具体的なソケット手順は実装 Issue で詰める。

### 上限の扱い

`MAX_SLOT_SIZE = 65536 byte (= 64 KiB)` を実装上限の初期値とする（暫定）。これにより:

- 1 driver あたり最大メモリ = `RING_CAPACITY (256) × MAX_SLOT_SIZE (65536) = 16 MiB`
- `slot_size > MAX_SLOT_SIZE` となる `max_payload_size`（具体的には `max_payload_size > 65528 byte`）の場合は **events.yaml の `bytes.max_length` を見直す方針**（圧縮するか、論理的に分割するか）

`slot_size > MAX_SLOT_SIZE` となる driver は handshake で reject され、起動できない。

---

## 戻り値仕様

`emit_event` 戻り値（本案で簡略化）:

| ケース | 戻り値 | 意味 |
|---|---|---|
| 成功 | `1` | リング slot に書けた |
| ring 満杯 | `0` | back-pressure（drop） |
| payload size 超過（payload バイト長 `> slot_size - 8`） | `-2` | events.yaml 違反 / driver 実装バグ |

ちょうど `slot_size - 8`（= 確定後の payload 領域上限）バイトの payload は **収まる**（境界条件は strict greater-than）。

side channel 案で必要だった「side channel フル」の概念がなくなる。`-2` の発生は実質 events.yaml validator の仕事で、handshake 時の `max_payload_size` 宣言が正しければ runtime で `-2` は出ない（防衛的に残すのみ）。

---

## メモリ予算

driver ごとの shm 使用量。`shm_total = sizeof(ShmHeader) (56) + RING_CAPACITY (256) × slot_size` を 4 KiB ページに切り上げた値が実 shm 容量。`slot_size` は `((max_payload_size + 8) + 3) & !3` で確定する。

| ユースケース | `bytes.max_length` の最大 | `slot_size`（確定値） | 実 shm 容量（ページ整列後） |
|---|---|---|---|
| MIDI（SysEx 1 KB 上限） | 1024 byte | 1032 byte | 260 KiB（`56 + 256 × 1032 = 264,248 byte → 65 ページ = 266,240 byte`）|
| OSC（典型的な float / int / 短い blob） | 256 byte | 264 byte | 68 KiB（`56 + 256 × 264 = 67,640 byte → 17 ページ = 69,632 byte`）|
| 大型 OSC blob 想定 | 4096 byte | 4104 byte | 1028 KiB（`56 + 256 × 4104 = 1,050,680 byte → 257 ページ = 1,052,672 byte`）|
| 仮想：長尺 SysEx | 16384 byte | 16392 byte | 4100 KiB（`56 + 256 × 16392 = 4,196,408 byte → 1025 ページ = 4,198,400 byte`）|
| 上限超過 | > 65528 byte | reject | 起動不可 |

> `max_payload_size > MAX_SLOT_SIZE - 8 byte = 65528 byte` のケースは reject される（`slot_size` が `MAX_SLOT_SIZE = 65536 byte (= 64 KiB)` を超えるため）。

`MAX_SLOT_SIZE = 65536 byte (= 64 KiB)` 上限なら、1 driver あたり最大 約 16 MiB。multi-driver 構成でも合計数十 MiB に収まる現実的な予算。

side channel 案との比較は [README.md](./README.md) 参照。

---

## メモリ順序

ring 既存の Acquire/Release で完結する（**side channel が無いので追加 fence 不要**）。

```text
Driver 側書き込み順:
  1. slot.payload に payload バイトを書く
  2. slot.payload_len, slot.occupied を書く
  3. ShmHeader.write_index を Release で進める

Bridge 側読み出し順:
  1. ShmHeader.write_index を Acquire で読む
  2. slot.payload_len, slot.occupied を読む
  3. slot.payload から payload バイトを読む
```

side channel 案にあった「`side_read_index` の独立 Release/Acquire」「ring と side の memory ordering 整合」のような複合的な議論が **不要**。

---

## API スケッチ

### Driver 側（midori-sdk 内）

```rust
// crates/midori-sdk/src/ring.rs（既存 spsc.rs を拡張する形を想定）

pub struct RingProducer {
    base: *mut u8,
    header: *mut ShmHeader,
    slot_size: u32,           // handshake で確定、ヘッダから読んだ値をキャッシュ
    write_index_local: u64,
}

impl RingProducer {
    /// Bridge から fd と slot_size を受け取って初期化。L1 から呼ぶ。
    pub unsafe fn from_fd(fd: RawFd) -> std::io::Result<Self>;

    /// payload を書き込み。
    /// `Err(Full)` = リング満杯（emit_event は `0` 戻り）、
    /// `Err(PayloadTooLarge)` = `slot_size - 8` 超（emit_event は `-2` 戻り、
    /// 通常は handshake で防御済）。
    pub fn write(&mut self, payload: &[u8]) -> Result<(), RingError>;
}

pub enum RingError {
    Full,
    PayloadTooLarge,
}
```

### L1 FFI（C ABI）

```c
/* 実装 Issue で確定。本書では存在のみを宣言 */

/* fd 受け渡し: handshake 完了後、Bridge から渡される fd を attach */
int midori_sdk_attach_ring(int fd);

/* handshake は別 API（control channel 経由）で確立済み前提 */
```

### Bridge 側（midori-runtime 内）

```rust
// crates/midori-runtime/src/ring.rs（新規ファイル想定）

pub struct RingConsumer {
    base: *const u8,
    header: *const ShmHeader,
    slot_size: u32,
}

impl RingConsumer {
    /// Driver からの handshake 要求 (max_payload_size) を受けて
    /// shm を確保し、fd を返す。
    pub fn create(max_payload_size: u32) -> std::io::Result<(Self, OwnedFd)>;

    /// 1 slot を pop し、payload を返す。
    pub fn read(&self) -> Option<Vec<u8>>;
}
```

---

## ABI / version

`ShmHeader.version: u32` 1 個のみで管理する。**side channel 案で必要だった `SideChannelHeader` は存在しないため、ABI 表面が 1 つ減る**。

policy は side channel 案と同じく:

- **初期版の `version` 値は `1`**（最初のリリースで Bridge が handshake 時にこの値を書き込み、`MIN_SUPPORTED_VERSION = MAX_SUPPORTED_VERSION = 1`）
- ABI に変化があれば minor / major 問わず必ず `version` を増分
- Bridge は `[MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION]` の範囲で受け入れ
- 末尾 `_pad` を消費する追加は append-only minor として古い driver も受け入れ続けられる

詳細表は [side-channel.md](./side-channel.md) の「ABI / version 取り扱い」節を参照（policy 自体は両案で共通）。

---

## 単一 Driver プロセス前提

`design/15-sdk-bindings-api.md`「実行インスタンス制約」と整合し、ring は **driver プロセスごとに独立した shm セグメント 1 個** を持つ。複数 driver で 1 個の ring をシェアしない。

---

## Out of Scope（再掲）

本書で **触らない** もの:

- handshake の具体的なソケット / control channel 実装
- mmap 確保コード・アンマップ手順
- L1 FFI の C ABI 詳細
- driver lifetime 中の slot_size 変更
- 圧縮、multi-producer
- テストケース具体記述

---

## 既存ドキュメントへの波及

採用された場合、実装 Issue で以下の更新が必要:

- `design/15-sdk-bindings-api.md`
  - 「SPSC スロットレイアウトの変更」の `RingSlot` 定義から `side_offset` / `side_len` を削除
  - 「`emit_event` の戻り値」節を本案の簡略化に合わせて更新
  - 「side channel」節を撤回し、本案へのリンクに置き換え
- `design/16-driver-events-schema.md`
  - 「SysEx の表現」節の `max_length: 1024` 由来コメントを本案の `slot_size` 説明にリンク
- `crates/midori-core/src/shm.rs`
  - `RingSlot` から `side_offset` / `side_len` / `_pad2` を削除
  - `payload` を動的サイズに変更（stride 計算に切り替え）
  - `ShmHeader` に `slot_size: u32` / `version: u32` / `_pad: [u8; 32]` を追加。**総サイズが 16 byte → 56 byte に拡大**
  - **RingSlot 配列の開始 offset が 16 → 56 へ変わる**：現状のモジュールコメント（`offset 16: slots[RING_CAPACITY]` と書かれている）と、ヘッダから slot を引き出す全コード（FFI 含む）を `size_of::<ShmHeader>()` で算出する形に修正
  - モジュールコメント（`//!`）の改訂：`side_offset` / `side_len` / `PAYLOAD_INLINE_MAX` への現存参照を **すべて削除** し、本書（`design/proposals/variable-ring.md`、または採用後の正式パス）への参照に置き換え。`offset 16: slots[...]` 行は新サイズ（56）または `size_of::<ShmHeader>()` 相対の表記へ更新
  - `PAYLOAD_INLINE_MAX` 定数および関連 `assert!` を削除
  - 既存テスト `shm_header_size_and_align`（`assert_eq!(size_of::<ShmHeader>(), 16)`）を **新サイズ 56 byte** へ更新（`align == 8` は据え置き）
  - 既存テスト `ring_slot_is_repr_c_and_fixed_size`（`assert!(size_of::<RingSlot>() == 264)`）は **削除**（`RingSlot` が動的サイズになるため固定サイズ assertion は意味を失う）
  - 代わりに `ShmHeader.slot_size` の最小値 / 4 byte 倍数性を runtime で検証する初期化ガードを追加
- `crates/midori-sdk/src/spsc.rs` / `crates/midori-sdk/src/ffi.rs`
  - `RingProducer` / `RingConsumer` の API に `slot_size` を導入
  - `midori_sdk_spsc_*` の C ABI を `slot_size` 受け取りに拡張（major bump）
  - **FFI 戻り値型の major breaking change**: 現在 `midori_sdk_spsc_push` / `midori_sdk_spsc_pop` は `u8` を返すが、本案で導入する `-2`（payload size 超過）の負値を表現するため **`int32_t`（または `int8_t`）** へ変更する必要がある。すべての言語ラッパー（PyO3 / napi-rs / C ヘッダ）も連動更新
- テスト全般（本リポジトリのテストは `crates/<name>/src/*.rs` 内の `#[cfg(test)] mod tests` に同居している。専用 `tests/` ディレクトリは現状無いので、改修は `src/` 内で完結する）
  - `crates/midori-sdk/src/spsc.rs` / `crates/midori-sdk/src/ffi.rs` の既存テスト（`PAYLOAD_INLINE_MAX` / 264 byte 固定 slot サイズに依存している箇所）を、`ShmHeader.slot_size` 由来の動的サイズ前提へ書き換え
  - SPSC ring を attach する SDK API（`RingProducer` / `RingConsumer`）と C ABI（`midori_sdk_spsc_*`）に `slot_size` 引数が増えたパスを通すテストを更新／追加
  - `ShmHeader.slot_size` の最小値 / 4 byte 倍数性を起動時に検証する unit test を `crates/midori-core/src/shm.rs` に新設（前述の runtime ガードと一対）

---

## 参考リンク

- [side-channel.md](./side-channel.md) — 案 A（比較対象）
- [README.md](./README.md) — 両案の比較と採否判断材料
- `design/15-sdk-bindings-api.md` — SDK バインディング API 設計
- `design/16-driver-events-schema.md` — events.yaml スキーマ（`bytes.max_length` の上限値定義）
- `crates/midori-core/src/shm.rs` — 現在の `RingSlot` 実装
