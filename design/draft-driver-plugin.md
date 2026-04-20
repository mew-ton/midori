# ドライバープラグイン構造 — 技術検討インデックス

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## ドキュメント一覧

| ファイル | 内容 |
|---|---|
| [draft-driver-sdk.md](./draft-driver-sdk.md) | ドライバーの概念・共有メモリ SPSC 通信・Driver SDK・多言語対応・バイナリ配布 |
| [draft-widget-render.md](./draft-widget-render.md) | ウィジェット（接続設定 UI）・描画コンポーネント（プレビュー外付け） |
| [draft-device-config-type.md](./draft-device-config-type.md) | Device Config Type の概念・osc-vrchat の立ち位置変更・プラグイン種別まとめ |

---

## 全体概要

```
プラグイン種別          実行形態                   イベント通信
────────────────────────────────────────────────────────────
ドライバー（全て）      サブプロセス               共有メモリ SPSC
  ↑ Driver SDK で開発。Rust / Python / Node.js / C++ 等で実装可能

Device Config Type      なし（YAML のみ）          —
  ↑ osc-vrchat はこれに分類

描画コンポーネント      GUI プロセス内 Web Component  dataset
  ↑ Shadow DOM 制約によりセキュリティを確保
```

ブリッジ本体はドライバー実装を持たない。MIDI・OSC も公式プラグインとして外部プロセスで動作する。
