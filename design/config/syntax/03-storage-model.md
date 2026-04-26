# 値の格納モデル — スキーマと実装の二層

値型システムは **スキーマ層** と **実装層** の二層に分かれる。スキーマ層では `static_array<T, N>` のように T をジェネリックに見せるが、実装層では T ごとに専用の格納戦略を持つ。両者の界面は config-load 時のモノモーフィック解決によって埋める。

[02-value-types.md](./02-value-types.md) が定義する値型はスキーマ層の表現であり、本ドキュメントはその裏で動く実装層の責務を整理する。

---

## 二層の責務

| 層 | 表現 | 責務 |
|---|---|---|
| スキーマ層 | YAML / 型システム | T に対して uniform。`static_array<bool, 16>` も `static_array<int, 16>` も同じ書き味で記述できる |
| 実装層 | ランタイムの格納実装 | T ごとに最適化された格納戦略。容量・密度・ホット経路の特性が個別に決まる |

スキーマ層は宣言的・開放的。実装層は閉じた型集合に対する specialized 実装の集まり。アダプター作成者・マッパー作成者は実装層を意識しないで済む。

---

## primitive ごとの格納戦略

各 primitive の自然な格納形:

| T | `static_array<T, N>` の物理表現 | `dynamic_array<T>` の物理表現 | 特殊事情 |
|---|---|---|---|
| `bool` | bit-packed ビット列 | 同左 + len | 8x 密度の旨味あり |
| `pulse` | bit-packed + tick 末リセットフック | 同左 | bool と分けるのは reset 責務のため |
| `int` | 連続メモリの整数配列 | 動的拡張可能な整数バッファ | range 駆動で要素幅 (i8 / i16 / i32 / i64) を狭める余地あり |
| `float` | 連続メモリの浮動小数配列 | 動的拡張可能な浮動小数バッファ | f32 / f64 の選択余地あり |

格納形が違えば再利用ポリシも違う。スカラーの primitive (`bool` / `pulse` / `int` / `float`) でも厳密にはタグ付き格納であり、ノード実装は型ごとに別実装を持つ。

`dynamic_array<T>` は名目上は「ランタイムで長さが変わる配列」だが、実装上は **入力長から上限が静的に算出される bounded scratch** として確保する。steady state では tick あたりのアロケーションを行わない。

---

## ノード実装の monomorphization

スキーマ層で名目上ジェネリックなノード (`array_merge` / `take` / `compact` 等) は、実装層では signature ごとに別実装を持つ。例:

- `array_merge<bool>` (bit OR で実装可能)
- `array_merge<int>` / `array_merge<float>` (要素ごと値コピー)
- `take<T>` (T ごとに格納の slice 取り方が異なる)

config-load 時の解決手順:

1. グラフ上の各ノードの T を型推論で確定
2. 対応する specialized impl を選択
3. ノードグラフを実装層のオブジェクトとして組み立てる

実装パターンは trait object dispatch でも enum dispatch でも構わない。primitive 数が小さく抑えられている範囲では enum dispatch のほうが見通しが良い。

ノード一覧 → [config/mapper-nodes/](../mapper-nodes/)

---

## 暗黙の型変換は行わない

実装層が type-specialized なため、暗黙の型変換は許可しない。型が異なるポート同士をつなぐには変換ノードを明示的に挟む。

このルールは [Layer 3 — 変換グラフ 要件](../../layers/03-mapper/requirements.md#型変換ルール) で既に明文化されている。本ドキュメントはその根拠となる実装側の事情を示す。

---

## 新規 primitive 追加のコスト

新しい primitive を追加するには、最低限以下を揃える必要がある:

1. **格納戦略の設計** — `static_array<T, N>` / `dynamic_array<T>` の物理表現。1 要素のバイト幅・上限・配置を確定する
2. **ドライバー / アダプター I/O 経路** — driver / adapter での読み書き・正規化処理
3. **関連ノードの specialized 実装** — `array_merge<T>` / `take<T>` / `compact<T>` 等を T 用に
4. **値型ドキュメントの更新** — [02-value-types.md](./02-value-types.md) と本ドキュメント

primitive を増やすほど実装層の specialized 実装が線形に増える。安易に増やさない。新しい primitive の追加は **格納戦略・I/O 経路・関連ノードを揃えてから**初めて initial scope に取り込む。

将来追加候補の primitive と導入指針は [05-future.md § 将来要件](../../05-future.md) を参照。

---

## 関連ドキュメント

- [config/syntax/02-value-types.md](./02-value-types.md) — primitive と配列型の値型定義 (スキーマ層)
- [layers/03-mapper/requirements.md](../../layers/03-mapper/requirements.md) — 型変換ルール
- [config/mapper-nodes/02-array.md](../mapper-nodes/02-array.md) — 配列操作ノード
- [05-future.md](../../05-future.md) — 将来追加候補の primitive
