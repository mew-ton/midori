# アイデア（掃き溜め）

> ステータス：未確定
> 最終更新：2026-05-08

## このフォルダの位置付け

設計判断として **確定していない思いつき・観察・スケッチ** を置く場所。確定設計ドキュメント（`design/` 直下の番号付きファイルや `layers/` 配下）と分離して保管することで、「決まったこと」と「ぼんやり眺めているだけのこと」が混在して読み手を混乱させるのを防ぐ。

ここのファイルは:

- **正典ではない**。実際に作る段になって覆ってよい。設計確定文書の一部として扱わない
- **完成を要求されない**。1 ファイルが 1 段落でもよい。論点だけ書いて結論なしでもよい
- **新しい確定情報の根拠にはならない**。本文書のどれかが本気で動き出すときは、確定設計の側にあらためて節を切り、出典としてここを参照しない（ここを参照すると「アイデアレベル」の事実が伝播するため）
- **確定設計ドキュメントから本フォルダへの inbound リンクは張らない**。リンクが張られると正典の一部のように扱われやすい

確定したらどうするか:

- 該当アイデアを起点に正典側に節を新設する
- このフォルダのファイルは削除するか、`Status: 確定済み（→ <移動先>）` の 1 行リダイレクトに差し替える

## 初期スコープ外ドライバー

初期実装では MIDI / OSC を双方向でサポートする。VRChat 用の OSC 設定は `osc-vrchat` アダプター種別定義として提供する。それ以外は将来拡張で、中身のスケッチは下記のファイルにある。

| ドライバー | 入力 | 出力 | スケッチ |
|---|---|---|---|
| MIDI | ✅ 初期実装 | ✅ 初期実装 | — |
| OSC（`osc`） | ✅ 初期実装 | ✅ 初期実装 | — |
| BLE Heart Rate | 将来 | — | — |
| WebSocket | 将来 | 将来 | — |
| HTTP | 将来 | 将来 | [`http-driver.md`](./http-driver.md) |
| Audio Spectrum | 将来 | — | [`audio-drivers.md`](./audio-drivers.md) |
| Audio Voice | 将来 | — | [`audio-drivers.md`](./audio-drivers.md) |

## 初期スコープ外 primitive

| primitive | 用途 | スケッチ |
|---|---|---|
| `char` | 文字データ（配信制作向けアダプター） | [`char-primitive.md`](./char-primitive.md) |

## ファイル一覧

| ファイル | 内容 |
|---|---|
| [cloud-runtime.md](./cloud-runtime.md) | ローカル MIDI / OSC をクラウドランタイムへ橋渡しする構成のメモ |
| [http-driver.md](./http-driver.md) | HTTP ドライバーの I/O モデルのスケッチ |
| [audio-drivers.md](./audio-drivers.md) | `audio-spectrum` / `audio-voice` 等のスケッチと粒度指標適用 |
| [char-primitive.md](./char-primitive.md) | `char` primitive の格納モデル・正規化・overflow ポリシ |
| [tick-vs-waveform.md](./tick-vs-waveform.md) | tick レートが波形系入力を吸収できるかという観察 |
| [unresolved.md](./unresolved.md) | 実機確認・調査が足りていない宿題（一時置き場、最終的には Issue 化） |
