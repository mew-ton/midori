# 検討中の設計案（proposals）

> ステータス：複数案を並走させて検討するための置き場
> 最終更新：2026-04-28

`design/` 直下に置かれるドキュメントは **採否が確定した仕様**。一方、本フォルダ（`design/proposals/`）には **複数案を並走させて比較検討中の設計案**を置く。

採用された案は本フォルダから抜き出して `design/NN-xxx.md` のような番号付きファイルに昇格させ、不採用案は本フォルダに残すか削除して履歴のみを残す。

---

## 現在検討中のテーマ

### Oversized Event Payload の配送方式

`RingSlot::payload` の `PAYLOAD_INLINE_MAX`（240 byte）を超える msgpack
payload（SysEx 1 KB 級・OSC blob 等）をどう運ぶか、2 案を比較検討中。

| 案 | ファイル | 概要 |
|---|---|---|
| **A. Side Channel** | [side-channel.md](./side-channel.md) | 別 mmap セグメントに oversized payload を逃す。ring は固定小サイズを維持 |
| **B. Variable Ring Slot** | [variable-ring.md](./variable-ring.md) | handshake 時に driver から `max_payload_size` を受け取り、ring 自体を必要サイズで確保。全 payload は inline |

#### 比較サマリ

メモリ列はいずれも **概算（オーダー比較用）**。案 B の正確な計算は ring slot に 8 byte ヘッダ + 4 byte align padding + ページ整列を加味した値で、実値は [variable-ring.md](./variable-ring.md) 「メモリ予算」節を参照。

| 観点 | A. Side Channel | B. Variable Ring Slot |
|---|---|---|
| **メモリ（MIDI 1 KB SysEx）** | 64 KiB ring + 256 KiB side ≒ **320 KiB** | ≒ **256 KiB**（実値は約 260 KiB） |
| **メモリ（典型 OSC 256 byte）** | 64 KiB ring + side 未使用 ≒ 64 KiB | ≒ 64 KiB（実値は約 68 KiB） |
| **メモリ（4 KB blob driver）** | 64 KiB ring + side（数 MiB） | ≒ 1 MiB（実値は約 1.004 MiB） |
| **shm セグメント数** | **2 個**（ring + side） | **1 個** |
| **fd 受け渡し** | 2 個 | 1 個 |
| **ABI 表面** | `ShmHeader` + `SideChannelHeader` の 2 ヘッダ | `ShmHeader` の 1 ヘッダ（`slot_size` / `version` 追加） |
| **ホットパス（典型イベント）** | inline で完結（cache friendly） | inline で完結（slot サイズが大きい分わずかに cache pressure） |
| **コールドパス（大きい payload）** | side channel 2 段 memcpy も発生 | 常に inline、追加コスト無し |
| **wrap-around / boundary handling** | あり（side channel 内） | **なし** |
| **GC / 寿命管理** | `side_read_index` を独立した Release/Acquire で同期 | ring の write/read index のみで完結 |
| **back-pressure シグナル** | ring 満杯 / side フルの 2 経路 | ring 満杯のみ |
| **handshake 拡張** | ring + side の 2 fd 受け渡しのみ | `max_payload_size` の事前宣言が必要 |
| **driver の責任** | events.yaml `bytes.max_length` 宣言 | 同上 + handshake で `max_payload_size` 通知 |
| **対応上限の柔軟さ** | `side_channel.capacity` を increase すれば 1 driver で大きな payload も対応可 | `MAX_SLOT_SIZE` 制約あり、超えると起動 reject |
| **実装複雑度** | 高 | 低 |
| **shm.rs / midori-core の変更** | `RingSlot` 維持・`SideChannelHeader` 新規追加 | `RingSlot` から side フィールド除去・`ShmHeader` 拡張 |

#### 採否判断の論点

1. **メモリ予算**: 案 B は worst-case を slot 数で乗算するため、`max_payload_size` が大きい driver では memory が膨らむ。MIDI/OSC で `1 KB` 程度なら案 B が **小さい / 同等**だが、`max_payload_size > 数 KB` になると逆転。
2. **ホットパス vs コールドパス比率**: typical event が数十 byte で稀に数 KB が混じる driver は案 A が cache 効率で有利。同サイズばかりなら案 B が単純で有利。
3. **複雑度コスト**: 案 A は wrap-around、独立 Release/Acquire、ABI ヘッダ 2 個と論点が多い。案 B は `slot_size` の handshake 1 点に集中。
4. **将来の拡張**: 案 A はサイドチャネル容量だけ広げれば柔軟（multi-driver 共有も理論上は可能）。案 B は driver lifetime 中の resize 不可。
5. **events.yaml validator との連動**: 案 B は handshake で payload size が確定するため、validator の責務がより明確になる。
6. **MIDI/OSC 以外の driver**: HID / Serial / Art-Net 等の将来追加で max payload 想定が大きく変わると案 B が苦しい可能性。

#### 現時点の傾き

midori の現在のユースケース（MIDI / OSC、`bytes.max_length <= 1 KB` 想定）では **案 B のほうがシンプルで memory も同等以下**。一方、将来の driver 種類拡張（特に大きな blob を扱うもの）が現実的に想定されるなら **案 A の柔軟さが効いてくる**。

採否は次のいずれかで判断:

- (1) ベンチマーク：両案の試作を作って実測（実装コストが高い）
- (2) ユースケース駆動：現実に想定される driver の `bytes.max_length` 分布から見積もる
- (3) 段階採用：最初は案 B（単純）→ 制約に引っかかったら案 A に移行

---

## proposals フォルダの運用

- 各案は **独立した markdown ファイル** として置く
- ファイル冒頭にステータス「**検討中（採否未決定）**」を明示する
- 比較表・採否判断材料は本 README に集約する（個別ドキュメントには書かない）
- 採用された案は `design/NN-xxx.md` に昇格させ、本フォルダの当該ファイルは削除（または `_archived/` 等へ移動）
- 不採用案は `_rejected/` 等へ移動するか、PR ベースで履歴に残す
