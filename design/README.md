# Midori 設計ドキュメント

> ステータス：設計フェーズ
> 最終更新：2026-04-28

## ドキュメント一覧

| ファイル | 内容 |
|---|---|
| [00-naming.md](./00-naming.md) | 用語・名前の定義（全ドキュメント共通） |
| [01-overview.md](./01-overview.md) | 概要・ユースケース・配布モデル・セキュリティ要件 |
| [02-architecture.md](./02-architecture.md) | 5層パイプライン・アダプター の対称性・Runtime/GUI 分離・リポジトリ構成 |
| [03-tech-stack.md](./03-tech-stack.md) | 技術スタック・Rust + Electron 構成・レイテンシ目標 |
| [04-runtime-cli.md](./04-runtime-cli.md) | CLI オプション・ログフォーマット |
| [05-future.md](./05-future.md) | 未解決事項・将来の拡張ポイント・参考リンク |
| [06-error-handling.md](./06-error-handling.md) | エラー分類・クリティカル／ランタイムエラーの挙動・GUI 可視化 |
| [07-ui-ux/](./07-ui-ux/) | 画面構成・遷移・各画面の UI 要件 |
| [08-ai.md](./08-ai.md) | AI アシスタント機能設計 |
| [09-plugin.md](./09-plugin.md) | プラグイン（アダプター・ドライバー・アダプター種別定義 の配布）仕様 |
| [10-driver-plugin.md](./10-driver-plugin.md) | ドライバー・アダプター種別定義・ウィジェット の概念と仕様 |
| [11-security/](./11-security/) | セキュリティ設計（セキュリティ水準・ドライバーサンドボックス・ウィジェット・AI） |
| [12-distribution.md](./12-distribution.md) | 配布窓口・アップデート方針・利用規約の枠組み |
| [13-ecosystem-readiness.md](./13-ecosystem-readiness.md) | エコシステム整備 TODO（契約・規約・導線の準備項目） |
| [15-sdk-bindings-api.md](./15-sdk-bindings-api.md) | SDK バインディング API 設計（C / Node.js / Python の L1/L2/L3 レイヤーモデル） |
| [16-driver-events-schema.md](./16-driver-events-schema.md) | Driver `events.yaml` スキーマ仕様（型語彙・binding_filter・SysEx・GUI 流用） |
| [17-driver-comm/](./17-driver-comm/) | Driver↔Bridge コミュニケーション戦略（tier モデル、inline ring 詳細、streamed 予約） |
| [proposals/](./proposals/) | 検討中の設計案（複数案を並走させて比較中。採用後は番号付きファイルへ昇格）|
| [layers/cross/timing.md](./layers/cross/timing.md) | tick 仕様・pulse リセット・MIDI タイミング |
| [config/00-component-types.md](./config/00-component-types.md) | component type 一覧（primitive value・必須フィールド・描画コンポーネント） |
| [config/01-preferences.md](./config/01-preferences.md) | Preferences（非配布・環境固有） |
| [config/02-adapter.md](./config/02-adapter.md) | アダプター 設定仕様（direction / definition / binding / layout） |
| [config/03-mapper.md](./config/03-mapper.md) | 変換グラフ 設定仕様（ノードグラフ・Signal 定義） |
| [config/05-profile.md](./config/05-profile.md) | プロファイル設定仕様（入出力デバイス・変換グラフ・接続設定） |
| [config/syntax/02-value-types.md](./config/syntax/02-value-types.md) | 値型リファレンス（bool / pulse / int / float / array） |
| [config/syntax/03-storage-model.md](./config/syntax/03-storage-model.md) | 値の格納モデル（スキーマ層と実装層の二層・新規 primitive 追加コスト） |
| [config/syntax/01-expr.md](./config/syntax/01-expr.md) | 式言語仕様（set.expr — SysEx 複数バイト計算） |
| [config/drivers/midi.md](./config/drivers/midi.md) | ドライバー仕様: MIDI（binding.input / binding.output） |
| [config/drivers/osc.md](./config/drivers/osc.md) | ドライバー仕様: OSC（binding.input / binding.output） |
| [layers/01-input-driver/requirements.md](./layers/01-input-driver/requirements.md) | Layer 1 入力ドライバー 要件 |
| [layers/02-input-recognition/requirements.md](./layers/02-input-recognition/requirements.md) | Layer 2 アダプター（入力）要件 |
| [layers/02-input-recognition/definition-requirements.md](./layers/02-input-recognition/definition-requirements.md) | Layer 2 definition 要件（component type 体系・additionals） |
| [layers/02-input-recognition/binding-requirements.md](./layers/02-input-recognition/binding-requirements.md) | Layer 2 binding 要件（raw events → ComponentState マッピング） |
| [layers/02-input-recognition/layout-requirements.md](./layers/02-input-recognition/layout-requirements.md) | Layer 2 layout 要件（View 描画定義） |
| [layers/02-input-recognition/signal-specifier.md](./layers/02-input-recognition/signal-specifier.md) | Signal 指定子 — definition から決まるパス文字列の仕様 |
| [layers/03-mapper/requirements.md](./layers/03-mapper/requirements.md) | Layer 3 変換グラフ 要件 |
| [layers/04-output-recognition/requirements.md](./layers/04-output-recognition/requirements.md) | Layer 4 アダプター（出力）要件 |
| [layers/05-output-driver/requirements.md](./layers/05-output-driver/requirements.md) | Layer 5 出力ドライバー 要件 |
