# Side Channel レイアウト案（検討中）

> ステータス：**検討中（採否未決定）**
> 最終更新：2026-04-28

本書は oversized event payload を運ぶ方式の **案 A**。**案 B（[variable-ring.md](./variable-ring.md)）** と比較検討中。両案の比較と採否判断材料は [README.md](./README.md) を参照。

`design/15-sdk-bindings-api.md`「SPSC スロットレイアウトの変更」で確保された
`RingSlot::side_offset` / `RingSlot::side_len` は、`PAYLOAD_INLINE_MAX`
（240 byte）を超える msgpack payload を逃すための「別 mmap 領域（**side
channel**）」を指すフィールドである。本ドキュメントはその side channel の
実体を **案 A** として規定する。

`design/16-driver-events-schema.md` の `bytes` 型 `max_length` 上限（SysEx で
1024 byte）は side channel の存在を前提に書かれており、本ドキュメントが完成
しないと **events.yaml の `max_length: 1024` がそのまま `RingSlot` に inline
できない時にどう運ばれるか** が未定義になる。

実装本体（mmap 確保コード・FFI 拡張・テスト）は本書のスコープ外。Driver/
Bridge 双方が触る API は **スケッチ** までを示し、実装 Issue で詳細化する。

---

## 全体方針

1. side channel は **SPSC ring と独立した mmap セグメント** として確保する
2. 割り当ては **wrap-around 単一バッファ**（リング型）。可変長バイト列を
   末尾境界をまたいで書き込み、Bridge が consume するごとに先頭から再利用
3. Driver / Bridge の同期は **ring の `write_index` / `read_index` と独立した
   `side_read_index` AtomicU64 ひとつ** で表現する（本書でいう `side_read_index`
   は構造体上は `SideChannelHeader::read_index` を指す。同一フィールド）
4. 領域フル時の `emit_event` 戻り値は **`0`（リング満杯と同じ back-pressure
   シグナル）** に統一する
5. 寿命は **driver プロセス単独**（同一プロセス内での複数 driver 並走は
   想定しない）

---

## スコープ

本ドキュメントで決めること:

- side channel の **物理配置**（mmap セグメントの分け方・ヘッダレイアウト）
- **総サイズの初期値**（運用で driver.yaml 上書き可能にするか否かは触る程度）
- **割り当て戦略**（wrap-around / split read / 単調インデックス）
- **寿命管理プロトコル**（Driver ↔ Bridge の `read_index` 受け渡し）
- **書き込み・読み出しのメモリ順序**
- **領域フル時の戻り値仕様**
- **Driver / Bridge 双方の API スケッチ**
- **ABI / version の取り扱い方針**

スコープ外（**Out of Scope**）:

- 実装本体（mmap 確保・FFI 拡張・Bridge 側パーサー実装）
- テストケースの具体記述
- driver.yaml で side channel サイズを上書きする仕様（拡張余地として記述する
  のみ。文法は実装 Issue 起票時に決める）
- multi-producer 化（SPSC 規律のみを想定）
- 圧縮（msgpack バイト列をそのまま運ぶ）

---

## 物理配置：別 mmap セグメント

### 選定：別セグメント

side channel は SPSC ring と **別の mmap セグメント** として確保する。

`crates/midori-core/src/shm.rs` の `ShmHeader` + `RingSlot[RING_CAPACITY]` が
入った既存セグメントとは独立した、新規 shm セグメント
（`SideChannelHeader` + バイトバッファ）。

### 採用理由

| 観点 | 同一セグメント（不採用） | 別セグメント（採用） |
|---|---|---|
| fd 受け渡し | 1 個 | 2 個（拡張は線形） |
| サイズ独立性 | ring と side が同じ shm 内なので resize が連動 | side のサイズだけ driver ごとに変えられる |
| レイアウトの将来変更 | ring or side のどちらかを変えると全体破壊 | 片方だけ semver 管理できる |
| ABI 単純さ | ヘッダが密結合 | 各セグメントが自分のヘッダで自己完結 |

将来 driver.yaml に `side_channel.capacity` の上書きを追加するときに、別
セグメントのほうが自然に拡張できる（ring は固定サイズで十分）。L1 が
Bridge から fd を受け取る時点で 2 個受け取る形にしておけば、3 個目以降の
shm（将来の追加チャネル）も同じ枠組みに乗せられる。

### fd 受け渡しプロトコル

`design/15-sdk-bindings-api.md`「後続 Issue 案」Phase 1 の `L1-2`（`midori_event_t`
ビルダー + msgpack encode → SPSC push と Bridge との fd 受け渡しプロトコル）
で扱う fd 受け渡し経路を拡張し、ring fd と side channel fd を **2 個まとめて
Driver に渡す** 形にする。具体的なソケット手順や ancillary data の構造は
当該 L1 実装 Issue で詰める（本書ではプロトコルが「fd 1 個 → 2 個」になる
ことだけを宣言）。

---

## 領域構造

### SideChannelHeader

```rust
// crates/midori-core/src/shm.rs に追加予定（実装 Issue で確定）
#[repr(C)]
pub struct SideChannelHeader {
    /// side channel ABI バージョン。**ABI に変化があれば minor / major
    /// 問わず必ず増分する**（「ABI / version 取り扱い」節の policy 参照）
    pub version: u32,
    /// `bytes` 領域の総バイト数。ページサイズアラインメント済み（4 KiB の倍数）
    pub capacity: u32,
    /// Bridge が consume 完了した最終バイト位置（モジュロ前の単調増加値）
    pub read_index: std::sync::atomic::AtomicU64,
    /// 将来フィールド追加余地（Cache line align）
    pub _pad: [u8; 48],
}

// 続く領域: bytes[capacity]（上記ヘッダの直後にページアラインで配置）
```

サイズ目安: `4 + 4 + 8 + 48 = 64 byte`（1 cache line）。続くバイトバッファは
ページサイズ（4 KiB）アラインで開始する。具体的な開始オフセットは

```
bytes_base_offset = align_up(sizeof(SideChannelHeader), PAGE_SIZE)
                  = align_up(64, 4096) = 4096
```

（mmap セグメントの 2 ページ目以降に配置）。

### バイトバッファ

ヘッダの直後に `capacity` バイトの領域を置く。Driver が msgpack バイト列を
そのまま書き込み、Bridge が読む。**書き込み・読み出しのオフセットは
モジュロ前（単調増加 u64）** で扱い、実アクセス時に `% capacity` で
buffer 内位置に変換する（後述「割り当て戦略」）。

### `RingSlot::side_offset` の解釈契約

`crates/midori-core/src/shm.rs` の `RingSlot::side_offset: u64` は、本書では
**モジュロ前の単調増加 u64**（side channel への通算書き込みバイト位置）として
扱う。実 mmap 内の位置は `side_offset % capacity` で取得する。
`side_offset == 0` は「side channel 先頭への最初の書き込み」のみで成立し、
2 周目以降は `side_offset >= capacity` となる（`% capacity` で先頭 0 に戻る）。

この解釈契約は本書で確定し、`crates/midori-core/src/shm.rs` の
`side_offset` フィールドの docstring もこの契約に合わせて改訂する
（後述「既存ドキュメントへの波及」参照）。

### `capacity` の型

`SideChannelHeader::capacity` はヘッダ上では `u32`（最大 4 GiB）として確保するが、
`SideChannelProducer` / `SideChannelConsumer` の内部状態としては **`u64`
にキャストして保持** する。`side_offset` が `u64` 単調値であり、`% capacity`
や残量計算（`(write_index_local - read_index)`）が `u64` 同士で完結するため。
ヘッダから読み込んだ直後にキャストし、以降は `u64` で扱う。

### 総サイズの初期値：256 KiB

| 計算根拠 | 値 |
|---|---|
| `RING_CAPACITY` | 256 |
| events.yaml `bytes.max_length`（SysEx 上限） | 1024 byte |
| 同時 outstanding 最大バイト数（理論上限） | 256 × 1024 = 256 KiB |
| ページアラインメント（4 KiB の倍数） | 256 KiB は満たす |

`RingSlot` 全スロットが side channel 経由 payload で埋まる最悪ケースを想定し
**256 KiB** を初期値とする。実運用では SysEx は数 % 程度で、ほとんどのスロット
は inline payload のため、256 KiB は余裕を持った見積もり。

将来 driver.yaml で `side_channel.capacity` を override する拡張を入れる場合
は、最低値 4 KiB（1 ページ）/ 最大値は実装制約（u32 の範囲内）で縛る。
本書ではここまで触れず、**実装 Issue で driver.yaml 文法を決める**前提。

---

## 割り当て戦略：wrap-around 単一バッファ

### 採用：wrap-around + 分割読み出し

リング型（先頭 wrap-around）の単一バッファとして扱う。書き込み中に末尾境界
を超える場合は **2 段書き込み**（前半は末尾、後半は先頭）で繋ぎ、Bridge も
`offset + len > capacity` を検出したら 2 段読みする。

オフセットは **モジュロ前の単調増加 u64**（`RingSlot::side_offset`）で
保持し、`% capacity` で実 buffer 内位置に変換する。

### 他案との比較

| 案 | 利点 | 欠点 | 採否 |
|---|---|---|---|
| **wrap-around + split**（本案） | バッファ無駄ゼロ、Driver/Bridge ともに 2 段アクセスに対応するだけ | 読み書きが境界をまたぐと 2 回 memcpy | ✅ 採用 |
| Padding（末尾を捨てて先頭から書き直し） | 読み出しは常に 1 回 | バッファの末尾を捨てる無駄、padding 量を Bridge にどう伝えるかでヘッダが膨らむ | ❌ |
| フリーリスト（malloc 風） | サイズ可変・断片化制御 | Driver/Bridge 両端でアロケータを実装、SPSC 規律と相性悪 | ❌ |
| バンプアロケータ（リセットなし） | 最速 | 容量 = 全 outstanding の累積、初回フル後は使えない | ❌ |

リング型は **SPSC 規律と相性が最も良い**：単一プロデューサーが書き込み位置を
単調増加し、単一コンシューマーが読み出し位置を単調増加する構造は、ring
buffer 既存の Acquire/Release パターンをそのまま流用できる。

### 書き込みアルゴリズム（Driver 側）

`write` の戻り値は **二系統のエラーを型で分ける**。`emit_event` 側の戻り値
仕様（`0` = back-pressure / `-2` = payload size 超過）と一対一対応させる
ため、`Result<(u64, u32), SideChannelError>` を返す:

```rust
pub enum SideChannelError {
    /// 領域フル（一過性）。emit_event は 0 を返す
    Full,
    /// payload が `u32::MAX` または `capacity` を超える。
    /// emit_event は -2 を返す（events.yaml 違反相当）
    SizeExceeded,
}

// 擬似コード（実装 Issue で実装）
fn write(&mut self, payload: &[u8]) -> Result<(u64, u32), SideChannelError> {
    let len = payload.len() as u64;
    if len > u32::MAX as u64 || len > self.capacity {
        return Err(SideChannelError::SizeExceeded);
    }

    let r = self.header.read_index.load(Acquire);
    let w = self.write_index_local;

    // 不変条件: w >= r（SPSC 規律により Driver 単一プロデューサーが
    // write_index_local を単調増加し、Bridge は r を w 以下の範囲でしか
    // 進めないため必ず成立）。実装では debug_assert! で確認する想定。
    debug_assert!(w >= r, "write_index_local must be >= read_index");

    // 残り空き容量
    let free = self.capacity - (w - r);
    if free < len {
        return Err(SideChannelError::Full);
    }

    // モジュロ前の単調値で side_offset を確定
    let side_offset = w;
    let off_in_buf = (w % self.capacity) as usize;
    let tail_room = self.capacity as usize - off_in_buf;

    if (len as usize) <= tail_room {
        // 1 回の memcpy で収まる
        copy(self.base.add(off_in_buf), payload);
    } else {
        // 末尾 tail_room バイト + 先頭 (len - tail_room) バイト
        copy(self.base.add(off_in_buf), &payload[..tail_room]);
        copy(self.base, &payload[tail_room..]);
    }

    // ring slot 側で Release されるので、ここでは local 更新のみ
    self.write_index_local = w + len;

    Ok((side_offset, len as u32))
}
```

`emit_event` 側はこれを次のようにマップする。**ring slot のフィールド設定
は排他規則** に従うこと（Bridge 側 `release_after_consume` が `side_len == 0`
で inline 判定するため、両者の同時設定や設定漏れは GC / back-pressure を
壊す）:

| ケース | `payload_len` | `side_offset` | `side_len` |
|---|---|---|---|
| inline（payload <= `PAYLOAD_INLINE_MAX`） | inline バイト長 | 不問 | **0** |
| side channel 経由 | **0** | `offset` | `len`（>0） |

```rust
match producer.write(&msgpack_bytes) {
    Ok((offset, len)) => {
        // canonical 設定（排他）:
        //   payload_len = 0 / side_offset = offset / side_len = len
        // ring slot に上記を書いた上で、ring の write_index を Release
    },
    Err(SideChannelError::Full) => return 0,         // back-pressure
    Err(SideChannelError::SizeExceeded) => return -2, // events.yaml 違反相当
}
```

**`SizeExceeded` は events.yaml の `bytes.max_length` チェック後に到達する
セーフティネット**。`emit_event` 内の検出順序（後述「back-pressure シグナル」
節）で `max_length` 超過は事前に弾かれるが、`max_length` 未指定の events
や Driver 実装のバグで `capacity` 超の payload が到達したら `write` 側で
最後の砦として `SizeExceeded` を返す。

`side_offset` は **モジュロ前の単調値**であり、`u64` の範囲なら 16 EB
（オーバーフローしない）。

### 読み出しアルゴリズム（Bridge 側）

```rust
// 擬似コード（実装 Issue で実装）
fn read(&self, side_offset: u64, side_len: u32) -> Vec<u8> {
    let off_in_buf = (side_offset % self.capacity as u64) as usize;
    let tail_room = self.capacity as usize - off_in_buf;
    let len = side_len as usize;

    if len <= tail_room {
        slice::from_raw_parts(self.base.add(off_in_buf), len).to_vec()
    } else {
        let mut v = Vec::with_capacity(len);
        v.extend_from_slice(slice::from_raw_parts(self.base.add(off_in_buf), tail_room));
        v.extend_from_slice(slice::from_raw_parts(self.base, len - tail_room));
        v
    }
}
```

Bridge は ring slot の `side_offset` / `side_len` をそのまま受け取り、
モジュロ計算と分割読みを自分で行う。Driver は `side_offset` をモジュロ前で
書き込むので、両端で `% capacity` の解釈が一致する。

### 末尾境界をまたぐ読み出しのコスト

「ring の 1 周ごとに高々 1 回」だけ 2 回読みが発生する。msgpack 1 KB
SysEx 程度の payload では 2 回 memcpy のオーバーヘッドは無視できる。MIDI
5000 events/s のうち SysEx は数 % のため、wrap 発生頻度は更に低い。

---

## 寿命管理 / GC プロトコル

### `side_read_index` 単調増加で寿命を表現

side channel の `read_index` は Bridge が更新する **モジュロ前の単調 u64** で、
意味は「**この値より前のバイトはすべて consume 済（再利用可）**」。

Bridge 側のリリース手順:

```rust
// ring slot を pop し、当該 payload バイトへの参照を破棄したら呼ぶ。
// decode / schema 照合の成否に関わらず必ず呼び出すこと
// （後述「consume 完了の境界」参照）。
fn release_after_consume(&self, slot: &RingSlot) {
    if slot.side_len == 0 {
        return; // inline payload のみ
    }
    let new_read_index = slot.side_offset + slot.side_len as u64;
    self.header.read_index.store(new_read_index, Release);
}
```

Driver 側は `read_index.load(Acquire)` で残り容量を計算する（前述
「書き込みアルゴリズム」参照）。

### ring 順序と side 順序の同一性

Driver は `emit_event` を L1 内 Mutex で直列化する（`design/15-sdk-bindings-api.md`
「スレッド / 非同期モデル」）。したがって、

- 1 回の `emit_event` の中で「side channel に書く → ring slot を立てる」が
  原子的に進む
- ring slot の順序（write_index 順）と side channel の書き込み順序
  （side_offset 単調増加）が **必ず一致** する

Bridge は ring を順番に pop するので、`side_offset` も単調増加で見える。
よって `read_index` を `slot.side_offset + slot.side_len` で順次更新するだけで
過不足なく side channel を解放できる（前のスロットの `side_len = 0` だった
場合も、次の side 利用スロットで `read_index` が一気に進むので問題ない）。

### consume 完了の境界

Bridge は **ring slot を pop 後、当該 payload バイトへの参照が不要になった
時点** で `read_index` を Release する。decode / schema 照合の **成否に
関わらず**、参照を破棄したら Release する。これは side channel の領域リーク
（恒常的 back-pressure 化）を防ぐための強制ルール。

```text
時系列（Bridge 側）:
  1. ring から pop（acquire ring write_index）
  2. side channel から (offset, len) を読む
  3a. msgpack decode → schema 照合 → Layer 2 binding 適用（成功）
  3b. decode / schema 照合に失敗 → エラーログを出して payload を捨てる
  4. side channel の read_index を Release で更新（3a / 3b いずれの経路でも実行）
```

**失敗経路（3b）でも必ず Release** するのが要点。`read_index` を進めずに
loop を抜けると、Driver 側から見て「side channel に空きがない」状態が
永続し、以降の `emit_event` がすべて `0`（back-pressure）を返す事態に
なる。decode 失敗時の payload は events.yaml 違反等で **そのバイト列に
意味は残らない** ので、参照を破棄して Release で OK。

ステップ 2〜3 の間に Driver が「同じバイト範囲」を上書きすることは **ない**：
Driver は `read_index` を見て「占有 = `write_index_local - read_index`」を
計算し、capacity を超えない範囲でしか書き込まないため。

ただし「Driver が書きたい時に空きがない場合」は back-pressure（次節）。

---

## back-pressure シグナル

### 戻り値仕様：`0` に統一

`design/15-sdk-bindings-api.md` の `emit_event` 戻り値仕様:

| ケース | 戻り値 | 意味 |
|---|---|---|
| 成功 | `1` | リング slot に書けた |
| **ring 満杯** | `0` | back-pressure（drop） |
| **side channel フル**（本書で確定） | `0` | back-pressure（drop）— **ring 満杯と同じ扱い** |
| payload size 超過（msgpack 後 > side channel `capacity` または > `events.yaml` の `bytes.max_length`） | `-2` | events.yaml 違反相当（Driver 作者バグ） |
| その他 | 既定通り | — |

> 注: ここでの戻り値は `midori_sdk_emit_event` のもの。`midori_sdk_run` も同じ
> `-2` 値を **IncompatibleSDK** の意味で別途持つが（`design/15-sdk-bindings-api.md`
> エラーモデル節参照）、関数が異なるため意味の競合は実害がない。両関数の
> 戻り値表を見比べる際は関数名で読み分けること。

### 同質判定の根拠

side channel フルは「Bridge 側が一過性に詰まっていて、消費が追いつかない」
状態で、ring 満杯と同じく **キャパシティ起因の一時的な back-pressure** に
分類される。Driver 作者から見て、ring 満杯と side channel フルを区別して
別ロジックを書く合理性は低く、両方とも「ドロップ件数をログに出すだけ」で
良い。

一方 payload size 超過（`-2`）は **events.yaml の `max_length` 設計と driver
コードのバグ**を示しており、運用中に起きるべきではない。両者を `0` と `-2` で
分けることで、Driver 作者は「`0` が頻発するなら ring/side を増やす」「`-2` が
出るなら events.yaml か emit コードを直す」という判定ができる。

### 検出順序

`emit_event` 内部の検出順序:

1. msgpack encode 後のバイト長が `events.yaml` の `max_length` または side
   channel `capacity` を超える → `-2`
2. `payload_len <= PAYLOAD_INLINE_MAX` なら inline 経路へ。ring 満杯なら `0`
3. それ以外（side channel 経由）→ side channel に空きがなければ `0`、
   書ければ ring slot を立てて push、ring 満杯なら `0`

ring と side のどちらが満杯でも `0` を返すため、Driver 作者は両者を区別する
必要がない。

---

## メモリ順序

### ring の Release/Acquire で side channel も保護される

side channel への payload 書き込みは「ring slot の `side_offset/side_len`
を立てる前」に完了している必要がある。具体的には:

```text
Driver 側書き込み順:
  1. side channel buffer に payload バイトを書く（**全 byte 書き終えるまで**
     ring slot を立てない。wrap-around で 2 段 memcpy になる場合は、
     **末尾領域への copy → 先頭領域への copy の両方が完了** していること）
  2. ring slot に (payload_len=0, side_offset, side_len) を書く
  3. ring の write_index を Release で進める

Bridge 側読み出し順:
  1. ring の write_index を Acquire で読む
  2. ring slot の side_offset / side_len を読む
  3. side channel buffer から payload バイトを読む（wrap-around で
     `(side_offset % capacity) + side_len > capacity` のときは
     2 段読み — 「割り当て戦略」節「読み出しアルゴリズム」参照）
```

ring 既存の Release/Acquire（`crates/midori-core/src/shm.rs` の `ShmHeader`
docstring 参照）が、ステップ 1 までに書かれた **すべてのストア**（ring slot
本体 + side channel buffer）を Bridge 側で可視化する。**side channel buffer
への独自の atomic fence は不要**。

### `side_read_index` の独立した Release/Acquire

side channel の `read_index` だけは ring とは別経路で Bridge → Driver に
伝わる:

| アクション | 主体 | ordering |
|---|---|---|
| `read_index.store(new_value, Release)` | Bridge | side channel buffer の payload 読み出し完了後に Release |
| `read_index.load(Acquire)` | Driver | 容量計算前に Acquire |

Driver の Acquire は「Bridge が既に読み終わった領域」を再利用する許可を
取るためのもの。書き込み中の領域とは独立しているので、ring 側のフェンスとは
切り離して扱える。

### Rust メモリモデルへの委譲

**Rust の `Acquire` / `Release` メモリオーダーを使えば追加のアーキ固有
fence を書く必要はない**。`AtomicU64::store(..., Release)` /
`AtomicU64::load(..., Acquire)` の Rust codegen が、各ターゲット
アーキテクチャ（x86 / ARM / RISC-V 等）で必要なバリア（無しを含む）を
出力する。実装側でアーキ判別して fence を入れ分ける必要は無い。

---

## Driver / Bridge API スケッチ

実装本体は別 Issue だが、本書で **API シグネチャの粒度を固定** しておく。
拡張余地（fd 個数、初期化引数）も含めてスケッチする。

### Driver 側（midori-sdk 内）

```rust
// crates/midori-sdk/src/side_channel.rs（新規ファイル想定）

/// Driver プロセスが Bridge から受け取る side channel ハンドル。
pub struct SideChannelProducer {
    base: *mut u8,                // mmap 先頭（バッファの先頭、ヘッダの直後）
    header: *mut SideChannelHeader,
    capacity: u64,                // ヘッダから読んだ値をキャッシュ
    write_index_local: u64,       // Driver 内のみで管理（Bridge には公開不要）
}

impl SideChannelProducer {
    /// Bridge から fd を受け取って mmap 経由で初期化。L1 から呼ぶ。
    pub unsafe fn from_fd(fd: RawFd) -> std::io::Result<Self>;

    /// payload を書き込み、(side_offset, side_len) を返す。
    /// `Err(Full)` = 領域フル（emit_event は `0` 戻り）、
    /// `Err(SizeExceeded)` = `u32::MAX` または `capacity` 超（emit_event は
    /// `-2` 戻り）。詳細は「書き込みアルゴリズム」節参照。
    pub fn write(&mut self, payload: &[u8])
        -> Result<(u64, u32), SideChannelError>;
}

pub enum SideChannelError {
    Full,
    SizeExceeded,
}
```

### L1 FFI（C ABI）

```c
/* 実装 Issue で確定。本書では存在のみを宣言 */

/* fd 受け渡し: 既存の ring fd プロトコルに side channel fd を追加 */
int midori_sdk_attach_side_channel(int fd);

/* 内部用（midori_sdk_emit_event から呼ばれる）。直接公開はしない */
```

### Bridge 側（midori-runtime 内）

```rust
// crates/midori-runtime/src/side_channel.rs（新規ファイル想定）

/// Bridge プロセスが driver ごとに保持する side channel ハンドル。
pub struct SideChannelConsumer {
    base: *const u8,
    header: *const SideChannelHeader,
    capacity: u64,
}

impl SideChannelConsumer {
    /// Driver プロセス起動時に Bridge 側で確保し、fd を Driver に渡す。
    pub fn create(capacity: u64) -> std::io::Result<(Self, OwnedFd)>;

    /// ring slot から (side_offset, side_len) を受け取り、payload を返す。
    /// アロケーションする (`Vec<u8>`) のは境界をまたぐケースのみで、
    /// 通常は zero-copy `&[u8]` を返す版も別途用意する。
    pub fn read(&self, side_offset: u64, side_len: u32) -> Vec<u8>;

    /// 該当 slot の consume を完了として読み位置を進める。
    pub fn release(&self, slot: &RingSlot);
}
```

### Driver↔Bridge ライフサイクル

| 段階 | 主体 | 操作 |
|---|---|---|
| 1. Bridge 起動 | Bridge | driver 1 つにつき ring 用 shm + side channel 用 shm を 2 個確保 |
| 2. Driver 起動 | Bridge | 2 個の fd を Driver プロセスに `SCM_RIGHTS` で渡す |
| 3. Driver 初期化 | Driver | 受け取った fd を mmap、`SideChannelProducer` を構築 |
| 4. 通常運用 | Driver | `emit_event` で必要に応じ side channel に書き込み、ring slot を立てる |
| 4'. 通常運用 | Bridge | ring slot を pop、必要なら side channel から読み出し、`read_index` を Release |
| 5. Driver 終了 | Driver | `SideChannelProducer` を drop（mmap unmap） |
| 6. Bridge 側終了 | Bridge | shm セグメント 2 個を unlink |

mmap unmap / shm unlink の具体手順は実装 Issue で確定。

---

## ABI / version 取り扱い

### policy 概要

side channel の ABI は `SideChannelHeader::version: u32` で表現し、**ABI に
何らかの変化があれば minor / major 問わず必ず `version` を増分する**。
Driver は「自分がビルドされた時点の `version`」を書き、Bridge は **自身が
受け入れる version 範囲** `[MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION]`
を持って判定する。

最初のリリースでは `MIN_SUPPORTED_VERSION = MAX_SUPPORTED_VERSION = 1`。
ABI が拡張されるたびに、Bridge 側で受け入れ可能な範囲を広げる。

### 受け入れマトリクス

| Driver 側 `version` | Bridge 判定 | 動作 |
|---|---|---|
| `version` < `MIN_SUPPORTED_VERSION` | reject | Bridge が知らない古いレイアウトのため。エラーログを出して当該 driver 起動を中止 |
| `MIN_SUPPORTED_VERSION` <= `version` <= `MAX_SUPPORTED_VERSION` | accept | Bridge は `version` 値を見て **どのフィールドまでが書かれているか** を判断 |
| `version` > `MAX_SUPPORTED_VERSION` | reject | Bridge が知らない新しいレイアウトのため。Bridge を更新するまで起動不可 |

### 互換性ルール

| 変更内容 | semver | `version` 更新 | 互換性 |
|---|---|---|---|
| 末尾の `_pad` を消費して新フィールド追加（既存フィールドの offset を変更しない） | **minor** | `version` を増分 | Bridge が `MAX_SUPPORTED_VERSION` を引き上げれば旧 driver も受け入れ続けられる（**append-only**） |
| 既存フィールドの型変更・順序変更・意味変更 | **major** | `version` を増分 | 旧 driver は `MIN_SUPPORTED_VERSION` の引き上げで reject される。`midori-core` の major bump とともに driver 再ビルド必須 |
| `capacity` のデフォルト値変更（レイアウト不変） | minor | 不要 | ABI 影響なし |
| バイトバッファの解釈変更（モジュロ単位、ヘッダレイアウトを変えずに意味だけ変える等） | **major** | `version` を増分 | レイアウト不変でも解釈差は ABI 互換性に影響するため major 扱い |

「append-only な minor bump」という規律により、Bridge が複数 version を
受け入れる実装を入れれば **旧 driver は再ビルドなしで動き続けられる** のが
本 ABI 設計の利点。

### validate_compat 擬似コード

```rust
// 擬似コード（実装 Issue で確定）
const MIN_SUPPORTED_VERSION: u32 = 1;
const MAX_SUPPORTED_VERSION: u32 = 1; // 拡張時に引き上げる

fn validate_compat(header: &SideChannelHeader) -> Result<(), CompatError> {
    let v = header.version;
    if v < MIN_SUPPORTED_VERSION || v > MAX_SUPPORTED_VERSION {
        return Err(CompatError::SideChannelVersionMismatch {
            actual: v,
            supported: MIN_SUPPORTED_VERSION..=MAX_SUPPORTED_VERSION,
        });
    }
    Ok(())
}
```

`crates/midori-core/src/shm.rs` の `ShmHeader` / `RingSlot` と同様、
`SideChannelHeader` のサイズは `const_assert!` でコンパイル時固定する
（実装 Issue で具体値確定）。

---

## 単一 Driver プロセス前提

`design/15-sdk-bindings-api.md`「実行インスタンス制約」で「同一プロセス内で
複数 driver の並走は別プロセス化」が原則として確定している。よって side
channel は **Driver プロセスごとに独立した shm セグメント 1 個** を持ち、
他 Driver と共有しない。

この前提により:

- `SideChannelProducer` は Driver プロセスごとに 1 個のみ存在
- L1 内部で SPSC ハンドルと並んで `SideChannelProducer` を 1 個だけ保持
- 複数 driver で 1 個の side channel をシェアする筋の悪い案は **採用しない**

将来「同一プロセス内で複数 driver」要求が出た場合、`design/15-sdk-bindings-api.md`
の「`midori_sdk_run_v2` で handle 引数を取る」拡張に合わせて、`SideChannelProducer`
も handle 単位に持たせれば対応できる。**現 ABI には影響しない**。

---

## Out of Scope（再掲）

本書で **触らない** もの:

- mmap 確保コード・アンマップ手順の具体実装
- L1 FFI（`midori_sdk_attach_side_channel` 等）の C ABI 詳細
- Bridge 側の SHM 接続経路（既存 IPC 実装との統合方針）
- side channel に乗せた payload の msgpack decode（events.yaml schema
  loader の責務）
- driver.yaml で `side_channel.capacity` を override する文法
- 圧縮（msgpack バイト列をそのまま運ぶ前提）
- multi-producer 対応
- テストケースの具体記述

これらは side channel 実装 Issue（`midori-core` / `midori-sdk` / `midori-runtime`
の実装本体）で扱う。

---

## 既存ドキュメントへの波及

本書の確定にともない、以下の記述を実装 Issue 着手時に更新する。**実装 Issue
の中で本書の改訂と整合させる**前提で、本書のスコープでは記述だけ列挙する。

- `design/15-sdk-bindings-api.md`
  - 「エラーモデル（言語別の決定）」節内の `emit_event` 戻り値段落に
    **side channel フル = `0`** を追記
  - 同段落の `-2` の発生条件を「`events.yaml` の `bytes.max_length` 超過 + side
    channel `capacity` 超過」に限定する旨を明記（「side channel が未実装 or
    拒否」「L3 の責任」表現の更新）
  - 「side channel 設計が固まるまで、Driver は SysEx を 240 byte 程度の inline
    範囲に収めること」の運用制約を撤回
  - 「side channel の設計（mmap 領域サイズ・割り当て・ガベージ）は本設計の
    スコープ外」の文言を「`design/17-side-channel.md` に確定」に更新
- `design/16-driver-events-schema.md`
  - 「SysEx の表現」節の `max_length: 1024` コメントの参照先を
    `design/17-side-channel.md` に変更
  - 「フィールド宣言の文法」表内の `max_length` 行で side channel への参照を
    必要に応じて追記
- `crates/midori-core/src/shm.rs`
  - モジュールコメント（`//!`）の「side channel 本体の確保・割り当て・GC は
    本ファイルでは扱わない」の参照先 doc 名を `design/17-side-channel.md` に
    明示
  - `RingSlot::side_offset` フィールドの docstring を **「モジュロ前の単調
    増加 u64。実 mmap 位置は `side_offset % side_capacity` で取得」** に改訂
    し、本書の解釈契約と一致させる（既存の「side channel 先頭を指す有効
    ポインタ」表現は実 mmap 位置解釈に誘導するため、本書の単調値解釈と
    両立する文言に改める）

---

## 参考リンク

- `design/15-sdk-bindings-api.md` — SDK バインディング API 設計（RingSlot
  新レイアウト・wire format・実行インスタンス制約）
- `design/16-driver-events-schema.md` — events.yaml スキーマ（`bytes.max_length`
  の上限値定義）
- `crates/midori-core/src/shm.rs` — `ShmHeader` / `RingSlot` / `PAYLOAD_INLINE_MAX`
  の確定実装
- `crates/midori-sdk/src/spsc.rs` — SPSC リング Producer/Consumer 実装（side
  channel API のリファレンス）
- `crates/midori-sdk/src/ffi.rs` — L1 FFI 実装（fd 受け渡し拡張のベース）
- `design/10-driver-plugin.md` — Driver プロセスモデル（プロセス分離原則）
