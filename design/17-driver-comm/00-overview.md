# Driver ↔ Bridge コミュニケーション戦略

> ステータス：設計フェーズ
> 最終更新：2026-04-28

driver プロセスから bridge（runtime）プロセスへ raw event を運ぶ経路と、その配送戦略を **tier 別に分離** する設計。本フォルダ（`design/17-driver-comm/`）は配送機構ごとに 1 ファイルで詳細を規定する。

実装本体（FFI 拡張・mmap 確保コード・bridge 側受信）は本フォルダのスコープ外。各 tier の **設計仕様** までを定める。

---

## 設計哲学

driver の event は性質ごとに要求が異なる:

- **速度を保証したい event**（MIDI noteOn など、sub-ms オーダーで届けたい）: 共有メモリの **固定長スロット** に押し込む。スロットサイズが固定なので allocate も発生せず、producer/consumer ともに極小コスト
- **速度より柔軟性が要る event**（OSC blob、将来の audio chunk、mapper 文字列など、サイズが大きい / 不定）: 速度保証を捨て、**shm 以外の経路**（pipe / socket / stdout 等）でストリームする

この責務分離を tier として明示する:

| Tier | 経路 | 速度保証 | サイズ柔軟性 | 詳細 |
|---|---|---|---|---|
| **inline** | shm SPSC ring（per-driver 別セグメント） | あり（固定 slot、allocate なし） | なし（slot_size 上限） | [01-inline-ring.md](./01-inline-ring.md) |
| **streamed**（将来） | shm 以外（具体プロトコル未定） | なし | あり | 未着手 |

---

## tier 宣言の所在

driver の `events.yaml` で **event 型ごとに `tier`** を宣言する（`design/16-driver-events-schema.md` の schema 拡張で `tier: inline | streamed` を追加予定）。default は `inline`。

```yaml
events:
  noteOn:                 # tier 省略 → inline (default)
    fields:
      channel: { type: midi_channel }
      note:    { type: uint7 }
      velocity: { type: uint7 }

  oscBlob:                # 大型 payload を扱う event は明示的に streamed
    tier: streamed
    fields:
      address: { type: string, max_length: 256 }
      payload: { type: bytes,  max_length: 65536 }
```

Bridge は events.yaml ロード時に各 event の tier を確定させ、tier ごとに別経路で配送する。

---

## 責任マトリクス

| Layer | 関心事 | tier への関与 |
|---|---|---|
| **Driver (Layer 1)** | event の語彙（型・フィールド・サイズ）と配送戦略 | events.yaml で `tier` を宣言する |
| **Bridge** | events.yaml に従った配送経路の選択、schema 照合 | tier を読んで内部配送先を決める |
| **Adapter (Layer 2 / 4)** | 抽象 component と raw event の値マッピング | **無関心**（event 型名で参照、配送機構は知らない） |

adapter 側の binding YAML には tier の語彙が現れない。これは「device は配送戦略まで責任を持つ、adapter は値の意味だけ扱う」という責務分離の帰結。

---

## limit 規約（inline tier）

inline tier の slot サイズは Bridge 側に 2 つの定数を持つ。両者ともに **slot 全体のバイト数**（ヘッダ 8 byte + payload + alignment padding を含む）を表す:

| 定数 | 値（暫定） | 役割 |
|---|---|---|
| `DEFAULT_SLOT_SIZE` | 1032 byte | driver から要求が無いときに確保する slot 全体サイズ。payload 容量は `1032 - 8 = 1024 byte` で MIDI SysEx 1 KiB 上限と一致 |
| `HARD_SLOT_SIZE` | 65536 byte (64 KiB) | driver 要求の上限。`slot_size > HARD_SLOT_SIZE` は handshake で reject |

Driver 側の振る舞い:

- inline tier の event ごとに **msgpack worst-case payload サイズ**（map / key / 各 value の worst-case を加算したもの）を算出し、event 間で最大値を `max_payload_size` とする
  - **算出規約の詳細**（map ヘッダ、各 type の msgpack worst-case 等）は [01-inline-ring.md](./01-inline-ring.md)「Handshake プロトコル」step 2 の表を **唯一の規範** とする。本書は summary
- 必要 `slot_size = ((max_payload_size + 8) + 3) & !3`（4 byte align）
- `slot_size <= DEFAULT_SLOT_SIZE`: handshake で要求しない（Bridge が default で確保）
- 超える: handshake で `slot_size` を要求。Bridge は `slot_size <= HARD_SLOT_SIZE` なら受理、超過なら reject

reject されたら driver 起動失敗。driver 作者は events.yaml の `bytes.max_length` を見直すか、該当 event を `tier: streamed` 化する。

メモリ予算: **1 driver あたり最大 `sizeof(ShmHeader) (56) + RING_CAPACITY (256) × HARD_SLOT_SIZE (65536) = 16,777,272 byte ≈ 16 MiB`**（ヘッダ 56 byte は誤差範囲）。合計メモリは driver 数 `N` に比例（最悪 `N × 16 MiB`、例: 4 driver で 64 MiB）。実値は driver ごとの実 `slot_size` で決まり、典型的には driver あたり数百 KiB に収まる。

詳細プロトコル・slot レイアウト・メモリ順序などは [01-inline-ring.md](./01-inline-ring.md)。

---

## streamed tier（将来）

**本フォルダで仕様予約のみ**、具体実装は未定。想定する性質:

- 配送経路: shm 以外（候補: pipe / unix socket / stdout JSONL を流用 等）
- 速度: ベストエフォート（リアルタイム保証なし）
- サイズ: `bytes.max_length` の硬い上限なし（実装側の現実的予算による）
- 主用途: OSC blob、audio chunk、文字列など inline 上限を超える可能性のある event

settle するまで、driver 作者は `tier: streamed` event を宣言しても **runtime はエラーで弾く**。

> 責務分離: `tier` 自体の構文妥当性（`inline | streamed` のいずれか、文字列として正しいか）は **events.yaml schema validator** が検証する。一方 `streamed` の **実利用可否**（Bridge が当該 tier を扱える capability を持つか）は **runtime feature-availability check** が判定し、未実装なら起動時に reject する。前者は静的、後者は動的検査。

実装着手時は本フォルダに `02-streamed.md` を新設し、本書の表もそこへ更新する。

---

## ファイル索引

- [01-inline-ring.md](./01-inline-ring.md) — inline tier の詳細仕様（variable-sized ring slot、handshake、ABI、メモリ順序）
- （将来）`02-streamed.md` — streamed tier の詳細仕様

---

## 参考リンク

- `design/15-sdk-bindings-api.md` — SDK バインディング API（C / Node / Python）と driver プロセスモデル
- `design/16-driver-events-schema.md` — events.yaml schema（`tier` 宣言の文法は本書と連動して別 Issue で追加予定）
- `design/10-driver-plugin.md` — Driver プラグイン仕様（プロセス分離原則）
- `crates/midori-core/src/shm.rs` — `RingSlot` / `ShmHeader` 実装
