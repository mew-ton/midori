# Inline Tier — Variable-Sized Ring Slot

> ステータス：設計フェーズ
> 最終更新：2026-04-28

本書は driver↔bridge 配送の **inline tier**（速度保証あり、shm SPSC ring 経由）の詳細仕様。tier モデル全体・他 tier との関係は [00-overview.md](./00-overview.md) を参照。

「driver が宣言する最大 payload サイズに合わせて、起動時 (handshake) に **RingSlot のサイズを動的に決める**」方針。各スロットは driver lifetime 内で固定サイズだが、driver プロセスごとに異なる。inline tier の event はすべて ring slot に inline で収まる。

実装本体（FFI 拡張・テスト）は本書のスコープ外。Driver/Bridge 双方が触る API は **スケッチ** までを示し、実装 Issue で詳細化する。

---

## 全体方針

1. inline tier の対象は events.yaml で `tier` 宣言が `inline`（または省略時の default）の event のみ。`tier: streamed` の event は本書の範囲外（[00-overview.md](./00-overview.md) 参照）
2. Bridge は **`DEFAULT_SLOT_SIZE` (1032 byte) / `HARD_SLOT_SIZE` (65536 byte)** の 2 つの slot 全体サイズ定数を持つ
3. driver は後述「Handshake プロトコル」step 2 の算出規約により `max_payload_size` を計算し、必要 `slot_size = ((max_payload_size + 8) + 3) & !3` を求める
   - `slot_size <= DEFAULT_SLOT_SIZE` のとき: **handshake で要求しない**。Bridge は default で確保
   - 超えるとき: **handshake で `slot_size` を要求**。Bridge は `slot_size <= HARD_SLOT_SIZE` なら受理、超えていれば reject
4. ring slot サイズは driver lifetime 内で **固定**。driver 再起動時のみ変えられる
5. 全 inline tier payload が **ring slot に inline 格納**。可変長の動的領域（旧 side_offset / side_len 等）は持たない
6. back-pressure はリング満杯のみ（`emit_event` が `0` を返す）。payload size 超過は handshake 時にハードに弾かれるため、`-2` の使い道は events.yaml 違反の防衛的検出のみ

---

## スコープ

本書で決めること:

- ハンドシェイクで `slot_size` を決めるプロトコル（`DEFAULT_SLOT_SIZE` / `HARD_SLOT_SIZE` の規約）
- `ShmHeader` への `slot_size` 追加と stride 計算
- `RingSlot` の固定 payload 配列前提の撤廃（payload 部が driver ごとに固定長だが driver 間で異なる）
- back-pressure 戻り値仕様の簡略化
- ABI / version の取り扱い方針

スコープ外（**Out of Scope**）:

- streamed tier の実装（[00-overview.md](./00-overview.md) で予約のみ）
- events.yaml schema への `tier` フィールド追加（別 Issue で `design/16-driver-events-schema.md` を改訂）
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

> **alignment 要件**: `slot_size` は **4 byte の倍数** であること（`payload_len: u32` の natural alignment を slot N >= 1 でも維持するため）。driver は **算出時に `slot_size = ((max_payload_size + 8) + 3) & !3` で丸める**。Bridge は受領した `slot_size` が 4 byte 倍数であることを **必ず検証** し、違反時は handshake を reject する（ABI 安全性の根幹のため driver 実装バグや不正入力に対する防衛）。公式に従えば alignment violation は自動的に防げる。

### ShmHeader レイアウト

```rust
#[repr(C)]
pub struct ShmHeader {
    pub write_index: AtomicU64,
    pub read_index: AtomicU64,
    pub slot_size: u32,        // バイト単位、handshake で決まる（4 byte 倍数）
    pub version: u32,          // ABI version（初期値 1）
    // 将来フィールド追加余地（append-only な version up 時に消費）。
    // 32 byte 確保しておくと、ヘッダ前段 24 byte と合わせて総 56 byte となり、
    // 64 byte cache line 1 本以内に収まる（残り 8 byte は将来追加分の余裕）
    pub _pad: [u8; 32],
}

// コンパイル時に size / alignment を lock:
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

### limit 規約

Bridge 側に 2 つの定数を持つ。両者ともに **slot 全体のバイト数**（ヘッダ 8 byte + payload + alignment padding を含む）を表す:

| 定数 | 値（暫定） | 役割 |
|---|---|---|
| `DEFAULT_SLOT_SIZE` | 1032 byte | driver 要求が無いときに確保する slot 全体サイズ。payload 容量は `1032 - 8 = 1024 byte` で MIDI SysEx 1 KiB 上限と一致するため、典型 driver は要求不要 |
| `HARD_SLOT_SIZE` | 65536 byte (64 KiB) | driver 要求の上限。`slot_size > HARD_SLOT_SIZE` は handshake で reject。1 driver あたり最大メモリ = `sizeof(ShmHeader) (56) + RING_CAPACITY (256) × HARD_SLOT_SIZE = 16,777,272 byte ≈ 16 MiB`（ヘッダ 56 byte は誤差範囲） |

両定数は将来 driver.yaml override の余地を残すが、初期は固定（実装 Issue で確定）。

### フロー

driver 起動時、Bridge 側で shm を確保するまでの流れ:

1. **driver 起動** → events.yaml をパース
2. **driver 側で `max_payload_size` と必要 `slot_size` を計算**（algorithm は実装間で deterministic に揃える）:
   - **`tier: inline`**（または default）の event のみ対象
   - **各 event について msgpack worst-case payload サイズを算出**（同一 event 内の可変長フィールドが**同時に**最大値を取るケースを採る = 過小見積もり防止）。算出規約は次の通り:

     | 要素 | worst-case サイズ |
     |---|---|
     | event 全体（msgpack map） | map ヘッダタグ（要素数 N に応じて: N≤15 で 1 byte / N≤2^16-1 で 3 byte / それ超は 5 byte）+ 各 key/value worst-case の総和 |
     | 各 key（msgpack str） | str ヘッダタグ（長さ L に応じて: L≤31 で 1 byte / L≤2^8-1 で 2 byte / L≤2^16-1 で 3 byte / それ超は 5 byte）+ key UTF-8 バイト長 |
     | `type` フィールドの value | str ヘッダタグ + driver が宣言する最長 type 文字列の UTF-8 バイト長 |
     | `bytes` / `string`（`max_length` 指定あり） | bin/str ヘッダタグ（`max_length` の値に応じた 1/2/3/5 byte）+ `max_length` |
     | 整数（uint7 / int7 / uint14 / int14 / nibble / midi_channel / int32 / int64） | 1 byte（fixint）または最大 9 byte（int64）。各型の最大に応じて固定 |
     | 浮動小数（float32 / float64） | 5 byte（float32）または 9 byte（float64） |
     | `bool` | 1 byte |
     | 配列（`max_length` 指定あり） | 配列ヘッダタグ（要素数に応じた 1/3/5 byte）+ 各要素 worst-case × `max_length` |

   - 全 inline tier event の中で **最大の worst-case サイズ** を `max_payload_size: u32` とする
   - **必要 `slot_size = ((max_payload_size + 8) + 3) & !3`**（ヘッダ 8 byte 加算後、`payload_len: u32` の natural alignment 維持のため 4 byte 倍数へ切り上げ。`!3` は bitwise NOT で `0xFFFF...FFFC` を生成し下位 2 bit をマスクするイディオム。算術等価式: `((max_payload_size + 8 + 3) / 4) * 4`）

   この algorithm は本書を **唯一の規範** とする（[00-overview.md](./00-overview.md) は summary で参照するのみ）。
3. **driver → Bridge** へ `request_ring(slot_size)` を送信（control channel 経由、handshake 必須メッセージ）
   - `slot_size <= DEFAULT_SLOT_SIZE` の場合: `slot_size = 0`（sentinel）を送信 → Bridge は DEFAULT で確保
   - 超える場合: 計算した `slot_size` を送信 → Bridge は受信値を採用（ただし上限チェックあり、step 4 参照）
4. **Bridge 側で受領**:
   - **slot_size の確定**:
     - 受信値が `0` (sentinel) → `slot_size = DEFAULT_SLOT_SIZE` (= 1032 byte)
     - それ以外 → 受信した `slot_size` をそのまま採用
   - **alignment 検証（必須）**: `slot_size % 4 != 0` なら reject（4 byte 倍数性違反は ABI 違反として扱う）
   - **上限チェック**: `slot_size > HARD_SLOT_SIZE` なら reject（events.yaml 見直し or `tier: streamed` 化を促す）
   - **shm 全体サイズの確定**: `shm_total = sizeof(ShmHeader) + RING_CAPACITY × slot_size`。これを **ページサイズ（4 KiB）で切り上げ** て allocate。具体式は `shm_mmap_size = (shm_total + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)`（`PAGE_SIZE = 4096`。mmap 単位がページなので shm 全体のみページ整列でよく、slot 単位の page-align は不要）
   - `ShmHeader.slot_size` に確定値を書き込み、`ShmHeader.version = 1`（初期版）で初期化
5. **Bridge → driver** に shm fd を返す
6. **driver は fd を mmap し、`ShmHeader.slot_size` を読み込んで stride 計算を確立**

control channel は `design/15-sdk-bindings-api.md` の Phase 1 / L1-2「Bridge との fd 受け渡しプロトコル」と統合する。具体的なソケット手順は実装 Issue で詰める。

### reject 時の挙動

`slot_size > HARD_SLOT_SIZE` で reject された driver は **起動できない**。driver 作者は次のいずれかで対応する:

- events.yaml の `bytes.max_length` を見直し、HARD 上限内に収める
- 大型 payload を扱う event を `tier: streamed` 化（streamed tier 実装後）
- payload を論理的に分割する（複数 event に切る）

`slot_size > HARD_SLOT_SIZE` となる driver は handshake で reject され、起動できない。

---

## 戻り値仕様

`emit_event` 戻り値:

| ケース | 戻り値 | 意味 |
|---|---|---|
| 成功 | `1` | リング slot に書けた |
| ring 満杯 | `0` | back-pressure（drop） |
| payload size 超過（payload バイト長 `> slot_size - 8`） | `-2` | events.yaml 違反 / driver 実装バグ |

ちょうど `slot_size - 8`（= 確定後の payload 領域上限）バイトの payload は **収まる**（境界条件は strict greater-than）。

`-2` の発生は実質 events.yaml validator の仕事で、handshake 時の `max_payload_size` 宣言が正しければ runtime で `-2` は出ない（防衛的に残すのみ）。

---

## メモリ予算

driver ごとの shm 使用量。`shm_total = sizeof(ShmHeader) (56) + RING_CAPACITY (256) × slot_size` を 4 KiB ページに切り上げた値が実 shm 容量。`slot_size` は handshake で確定（前述）。

| ユースケース | `bytes.max_length` の最大 | handshake | `slot_size`（確定値） | 実 shm 容量（ページ整列後） |
|---|---|---|---|---|
| MIDI（SysEx 1 KB 上限） | 1024 byte | 要求なし（default 内） | 1032 byte | 260 KiB（`56 + 256 × 1032 = 264,248 byte → 65 ページ = 266,240 byte`）|
| OSC（典型的な float / int / 短い blob） | 256 byte | 要求なし | 1032 byte | 260 KiB（同上、default で確保） |
| 大型 OSC blob 想定 | 4096 byte | 要求あり | 4104 byte | 1028 KiB（`56 + 256 × 4104 = 1,050,680 byte → 257 ページ = 1,052,672 byte`）|
| 仮想：長尺 SysEx | 16384 byte | 要求あり | 16392 byte | 4100 KiB（`56 + 256 × 16392 = 4,196,408 byte → 1025 ページ = 4,198,400 byte`）|
| HARD 超過 | > 65528 byte（slot_size > 65536 となる） | reject | — | 起動不可 |

> default 内の driver は handshake で `slot_size` を要求しないため、Bridge 側で `DEFAULT_SLOT_SIZE = 1032 byte` の slot で確保される（OSC のように `max_payload_size` が default より小さい場合も同じ slot サイズ）。
> HARD 上限なら 1 driver あたり最大 `RING_CAPACITY (256) × HARD_SLOT_SIZE (65536) = 16 MiB` ちょうど。合計メモリは driver 数 `N` に比例（最悪 `N × 16 MiB`、典型は driver あたり数百 KiB なので大半の運用で数十 MiB に収まる）。

---

## メモリ順序

ring 既存の Acquire/Release で完結する（**inline tier は ring 単独で動くため追加 fence 不要**）。

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

追加の atomic フィールドや別経路との memory ordering 整合の議論は **不要**（ring 単独で完結）。

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

`ShmHeader.version: u32` 1 個のみで管理する。**この値は単調増加する整数カウンタ**であり、semver の major/minor をエンコードしたものではない（互換性は Bridge 側の `MIN_SUPPORTED_VERSION` / `MAX_SUPPORTED_VERSION` レンジで判定する）。

### policy 概要

- **初期版の `version` 値は `1`**（最初のリリースで Bridge が handshake 時にこの値を書き込み、`MIN_SUPPORTED_VERSION = MAX_SUPPORTED_VERSION = 1`）
- ABI に変化があれば変更種別を問わず必ず `version` を `+1` 増分（u32 カウンタなので semver のような major/minor の符号化はしない）
- Bridge は `[MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION]` の範囲で受け入れ
- **append-only な変更**（末尾 `_pad` 消費でフィールド追加）は `MAX_SUPPORTED_VERSION` を引き上げるだけで旧 driver も受け入れ続けられる
- **breaking な変更**（既存フィールドの型・順序・意味の変更）は `MIN_SUPPORTED_VERSION` を引き上げて旧 driver を reject する

### 受け入れマトリクス

| Driver 側 `version` | Bridge 判定 | 動作 |
|---|---|---|
| `version` < `MIN_SUPPORTED_VERSION` | reject | Bridge が知らない古いレイアウトのため。エラーログを出して当該 driver 起動を中止 |
| `MIN_SUPPORTED_VERSION` <= `version` <= `MAX_SUPPORTED_VERSION` | accept | Bridge は `version` 値を見て **どのフィールドまでが書かれているか** を判断 |
| `version` > `MAX_SUPPORTED_VERSION` | reject | Bridge が知らない新しいレイアウトのため。Bridge を更新するまで起動不可 |

### 互換性ルール

| 変更内容 | 変更タイプ | `version` 更新 | 互換性 |
|---|---|---|---|
| 末尾の `_pad` を消費して新フィールド追加（既存フィールドの offset を変更しない） | **append-only** | `version` を増分 | Bridge が `MAX_SUPPORTED_VERSION` を引き上げれば旧 driver も受け入れ続けられる |
| 既存フィールドの型変更・順序変更・意味変更 | **breaking** | `version` を増分 | 旧 driver は `MIN_SUPPORTED_VERSION` の引き上げで reject される。`midori-core` の major bump とともに driver 再ビルド必須 |
| `slot_size` 既定値の変更（レイアウト不変） | non-ABI | 不要 | ABI 影響なし |
| バイトバッファの解釈変更（stride 計算の意味変更等） | **breaking** | `version` を増分 | レイアウト不変でも解釈差は ABI 互換性に影響するため breaking 扱い |

「append-only な minor bump」という規律により、Bridge が複数 version を受け入れる実装を入れれば **旧 driver は再ビルドなしで動き続けられる** のが本 ABI 設計の利点。

### validate_compat 擬似コード

```rust
// 擬似コード（実装 Issue で確定）
const MIN_SUPPORTED_VERSION: u32 = 1;
const MAX_SUPPORTED_VERSION: u32 = 1; // 拡張時に引き上げる

fn validate_compat(header: &ShmHeader) -> Result<(), CompatError> {
    let v = header.version;
    if v < MIN_SUPPORTED_VERSION || v > MAX_SUPPORTED_VERSION {
        return Err(CompatError::ShmVersionMismatch {
            actual: v,
            supported: MIN_SUPPORTED_VERSION..=MAX_SUPPORTED_VERSION,
        });
    }
    Ok(())
}
```

`ShmHeader` のサイズは `crates/midori-core/src/shm.rs` で `const_assert!` でコンパイル時固定する（実装 Issue で具体値確定）。

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

## 参考リンク

- [00-overview.md](./00-overview.md) — driver↔bridge 配送戦略の総論（tier モデル、limit 規約）
- `design/15-sdk-bindings-api.md` — SDK バインディング API 設計
- `design/16-driver-events-schema.md` — events.yaml スキーマ（`bytes.max_length` の上限値定義 / `tier` 宣言）
- `crates/midori-core/src/shm.rs` — `RingSlot` / `ShmHeader` 実装
