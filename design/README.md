# Midori 設計ドキュメント

> ステータス：設計フェーズ
> 最終更新：2026-04-04

## ドキュメント一覧

| ファイル | 内容 |
|---|---|
| [01-overview.md](./01-overview.md) | 概要・ユースケース・配布モデル・セキュリティ要件 |
| [02-architecture.md](./02-architecture.md) | 5層パイプライン・Device Profile の対称性・Runtime/GUI 分離・リポジトリ構成 |
| [03-tech-stack.md](./03-tech-stack.md) | 技術スタック・Rust + Electron 構成・レイテンシ目標 |
| [04-runtime-cli.md](./04-runtime-cli.md) | CLI オプション・ログフォーマット |
| [05-future.md](./05-future.md) | 未解決事項・将来の拡張ポイント・参考リンク |
| [config/01-preferences.md](./config/01-preferences.md) | Preferences（非配布・環境固有） |
| [config/02-input-source-profile.md](./config/02-input-source-profile.md) | Device Profile（入力）設定仕様（direction / definition / binding / layout） |
| [config/03-mapper.md](./config/03-mapper.md) | Mapper 設定仕様（ノードグラフ・Signal 定義） |
| [config/04-output-target-profile.md](./config/04-output-target-profile.md) | Device Profile（出力）設定仕様（direction / definition / binding / layout） |
| [layers/01-input-driver/requirements.md](./layers/01-input-driver/requirements.md) | Layer 1 入力ドライバー 要件 |
| [layers/02-input-recognition/requirements.md](./layers/02-input-recognition/requirements.md) | Layer 2 Device Profile（入力）要件 |
| [layers/03-mapper/requirements.md](./layers/03-mapper/requirements.md) | Layer 3 マッパー 要件 |
| [layers/04-output-recognition/requirements.md](./layers/04-output-recognition/requirements.md) | Layer 4 Device Profile（出力）要件 |
| [layers/05-output-driver/requirements.md](./layers/05-output-driver/requirements.md) | Layer 5 出力ドライバー 要件 |
