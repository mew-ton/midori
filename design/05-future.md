# 未解決・将来の検討事項

このドキュメントは **初期スコープ外であることが事実として確定している項目** と **未解決の調査 TODO** を列挙する。各項目の中身のスケッチ（I/O モデル・データ表現・運用論点）は確定設計ではなくアイデアの段階なので [`ideas/`](./ideas/) に分散して保管する。

## 将来要件（初期スコープ外）

初期実装には含めないが、設計上の拡張ポイントとして念頭に置く要件。

### 追加ドライバー

初期実装では MIDI / OSC を双方向でサポートする。VRChat 用の OSC 設定は `osc-vrchat` アダプター種別定義 として提供する。追加ドライバーは将来拡張。

| ドライバー | 入力 | 出力 | スケッチ |
|---|---|---|---|
| MIDI | ✅ 初期実装 | ✅ 初期実装 | — |
| OSC（`osc`） | ✅ 初期実装 | ✅ 初期実装 | — |
| BLE Heart Rate | 将来 | — | — |
| WebSocket | 将来 | 将来 | — |
| HTTP | 将来 | 将来 | [`ideas/http-driver.md`](./ideas/http-driver.md) |
| Audio Spectrum | 将来 | — | [`ideas/audio-drivers.md`](./ideas/audio-drivers.md) |
| Audio Voice | 将来 | — | [`ideas/audio-drivers.md`](./ideas/audio-drivers.md) |

`osc-vrchat` は独立ドライバーではなく、`osc` ドライバーを基底とする **アダプター種別定義** として提供する。詳細 → [`10-driver-plugin.md`](10-driver-plugin.md)

### 追加 primitive

| primitive | 用途 | スケッチ |
|---|---|---|
| `char` | 文字データ（配信制作向けアダプターで必要） | [`ideas/char-primitive.md`](./ideas/char-primitive.md) |

## 未解決事項

| 項目 | 内容 |
|---|---|
| ELS-03 チャンネルマップ | 実機確認が必要。判明後 `els03.yaml` の binding に反映 |
| ELS-03 キー横傾きの MIDI 実装 | MPE / チャンネル PitchBend / SysEx のいずれかを実機確認で特定 |
| 変換グラフ の複合ロジック | 和音検出は現時点で対応外。将来拡張ポイント |
| OSCQuery 対応 | VRChat 起動中にアバターパラメーターをリアルタイム取得。初期実装はローカルファイル読み取りで代替 |
| 追加入力ドライバー | `ble-heart-rate`, `keyboard`, `osc-input` など |
| 追加出力ドライバー | `websocket`, `serial` など |

## 参考リンク

- [Yamaha ELS-03 MIDI リファレンス](https://jp.yamaha.com/) — 機種別 PDF（実機確認要）
- [VRChat OSC Avatar Parameters](https://docs.vrchat.com/docs/osc-avatar-parameters)
- [VRChat OSC Resources](https://docs.vrchat.com/docs/osc-resources)
- [VRChat OSCQuery](https://docs.vrchat.com/docs/oscquery)
- [midir（Rust MIDI ライブラリ）](https://github.com/Boddlnagg/midir)
- [rosc（Rust OSC ライブラリ）](https://github.com/klingtnet/rosc)
- [Electron](https://www.electronjs.org)
- [Astro](https://astro.build)
