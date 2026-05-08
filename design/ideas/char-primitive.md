# `char` primitive

> ステータス：アイデアレベル（スケッチが進んでいる）
> 最終更新：2026-05-08

文字データを扱うための primitive。配信制作向けアダプター（obs-websocket / vtube-studio / twitch-eventsub / spotify-now-playing / pixoo / 等）で必要になる。初期スコープには含めず、本節の指針に従って後段で追加する。

格納コストの一般則は [`../config/syntax/03-storage-model.md`](../config/syntax/03-storage-model.md) を参照。

## char の単位

`char` は **NFC 正規化済みの 1 grapheme cluster** を表す opaque 型として定義する。

- 算術演算（加減算・bit 演算）は持たない
- 利用可能な操作は (a) `char[]` 全体を文字列処理ノード（concat / format / regex / equals / hash / take / 等）に渡す、(b) N 番目の要素を抽出して `char` として取り出す、の 2 種に限る
- truncate / length / indexing が「人間が見る 1 文字」の単位で動くことを保証する（絵文字・結合文字・国旗・肌色修飾子で破綻しない）

## 格納モデル

- 内部表現は **NFC UTF-8 バイト列 + grapheme 境界インデックス**
- 1 grapheme の最大バイト幅は `MAX_GRAPHEME_BYTES`（既定 64）で打ち切る。超過時は driver / adapter 入口で強制境界分割する
- `static_array<char, N>` の物理メモリ予算は `N * MAX_GRAPHEME_BYTES`
- ASCII 確定フィールド向けに `char[N, ascii]` 修飾で `MAX_GRAPHEME_BYTES = 1` を強制する選択肢を持つ

`char` を opaque に保つ限り「1 要素のバイト幅が可変」であっても [`../config/syntax/03-storage-model.md`](../config/syntax/03-storage-model.md) の二層モデルは崩れない。算術ノードに `char` を流通させない、という制約だけを守る。

## NFC 正規化

driver / adapter で受信した文字データは **入口で 1 回だけ NFC 正規化を行ってからマッパー層へ流す**。マッパー内部に流通する文字データは常に NFC 形と仮定でき、equals / hash / regex の比較が char-by-char で安定する。

採用する **Unicode バージョン** と **正規化実装ライブラリ** は char 実装 PR の時点で固定し、ランタイムの定数・設定として記録する。Unicode バージョンが上がった際は、grapheme segmentation 規則 (`MAX_GRAPHEME_BYTES` の妥当性含む) と正規化結果の差分が profile / adapter の互換性に与える影響を明示してから上げる。

## 文字データの境界規律

文字データの長さに関する規律は **境界（driver / 出力ポート）でだけ強制し、マッパー内部は自由**とする:

| 経路 | 許可される型 |
|---|---|
| driver / adapter からマッパーへの流入 | `static_array<char, N>` のみ。`N` は adapter kind が宣言する |
| マッパーグラフ内部 | `static_array<char, N>` および `dynamic_array<char>` の両方を流通可 |
| 出力ブロックのポート | `static_array<char, N>` のみ。`dynamic_array<char>` の到達時はオーバーフローポリシで bounded バッファに書き写す |

`dynamic_array<char>` は実装上、入力長から上限が静的に算出される bounded scratch として確保され、steady state ではアロケーションを行わない（[`../config/syntax/03-storage-model.md`](../config/syntax/03-storage-model.md) の `dynamic_array<T>` 一般則の char への適用）。

## オーバーフローポリシ

`on_overflow` フィールドで指定する。`out_of_range`（数値版）の char 版に相当する:

| 値 | 挙動 | 用途 |
|---|---|---|
| `truncate` | USV 境界で切る | 内部処理・ログ・ASCII 想定フィールド |
| `truncate_grapheme` | grapheme cluster 境界で切る（N USV を超えない最大長） | 表示系（Pixoo / OBS テキスト / Stream Deck ボタンラベル 等） |
| `drop_event` | イベント自体を破棄 | 不正データ排除 |
| `error` | バリデーションエラー / イベント skip | UUID 等の長さが正確である必要があるフィールド |

表示系アダプターは `truncate_grapheme` を既定とする。

## 例外: 長さ無制限の入力

「いくらでも長くしてよい何らかの機能から文字列をもらう」ケースは例外として扱う。アダプター kind が明示的に `unbounded_text` capability を宣言した場合のみ許可し、当該アダプターは出力エンドポイントとしてのみ接続できる（マッパー通常 signal としては流通させない）。TTS のような端点用途を想定する。

## 関連ノード（char と同時に追加）

- 生成: `format` / `concat`
- 分解: `split` / `substring` / `regex_capture`
- 検査: `equals` / `regex_match` / `starts_with` / `contains`
- 変換: `parse_int` / `parse_float` / `to_string` / `to_lower` / `to_upper` / `trim`
- ディスパッチ: `setMap.map` の char キー対応

## 導入時のチェックリスト

[`../config/syntax/03-storage-model.md` § 新規 primitive 追加のコスト](../config/syntax/03-storage-model.md#新規-primitive-追加のコスト) に従い、以下を揃えてから initial scope に取り込む:

1. UTF-8 + grapheme 境界インデックスの格納実装
2. Unicode 正規化（NFC）と grapheme segmentation の依存追加
3. **採用 Unicode バージョンと正規化／segmentation ライブラリの固定**（ランタイム定数として記録。アップグレード手順を運用ドキュメントに併記）
4. driver / adapter 入口の NFC 正規化処理
5. 上記関連ノードの specialized 実装
6. アダプター kind スキーマの `char[N]` / `on_overflow` 宣言サポート
7. [`../config/syntax/02-value-types.md`](../config/syntax/02-value-types.md) の primitive 表更新

## 適用ユースケース（動機）

- OBS / vMix のシーン名・ソース名による指定
- VTube Studio のホットキー UUID 指定
- Twitch チャット / Channel Points / Redeem の文字列入力
- Now Playing 系（Spotify 等）の楽曲・アーティスト名表示
- Pixoo / Divoom 等の物理ディスプレイへのテキスト送出
- SysEx パッチ名・ソング名の表示連携

これらのアダプターは [`../09-plugin.md`](../09-plugin.md) のプラグイン枠で順次追加する。
