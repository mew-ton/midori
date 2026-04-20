# 未解決・将来の検討事項

## 将来要件（初期スコープ外）

初期実装には含めないが、設計上の拡張ポイントとして念頭に置く要件。

### 追加ドライバー

初期実装では MIDI / OSC を双方向でサポートする。VRChat 用の OSC 設定は `osc-vrchat` デバイス種別定義 として提供する。追加ドライバーは将来拡張。

| ドライバー | 入力 | 出力 |
|---|---|---|
| MIDI | ✅ 初期実装 | ✅ 初期実装 |
| OSC（`osc`） | ✅ 初期実装 | ✅ 初期実装 |
| BLE Heart Rate | 将来 | — |
| WebSocket | 将来 | 将来 |
| HTTP | 将来 | 将来 |

`osc-vrchat` は独立ドライバーではなく、`osc` ドライバーを基底とする **デバイス種別定義** として提供する。詳細 → [`10-driver-plugin.md`](10-driver-plugin.md)

#### HTTP ドライバーのイメージ

HTTP はドライバー固有の I/O モデルを持つ：

**入力（サーバー起動型）**：ブリッジ起動時に HTTP サーバーが指定ポートで立ち上がる。
デバイス構成 の `definition` は受け付ける API エンドポイントを記述し、`binding` でリクエストボディのフィールドを ComponentState にマッピングする。

```yaml
# 入力 デバイス構成（driver: http）のイメージ
definition:
  components:
    - id: note_trigger
      type: pulse
    - id: expression
      type: slider
      range: [0, 1]

binding:
  input:
    driver: http
    mappings:
      - from:
          method: POST
          path: /note
          body: $.note        # JSON パス
        to:
          target: note_trigger.triggered
          set: 1
      - from:
          method: POST
          path: /expression
          body: $.value
        to:
          target: expression.value
          set: value
```

**出力（HTTP クライアント型）**：Signal が発生するたびにプロファイルの connection で設定した URL へ JSON body をリクエスト送出する。

```yaml
# 出力 デバイス構成（driver: http）のイメージ
binding:
  output:
    driver: http
    mappings:
      - from:
          target: upper.{note}.pressed
        to:
          method: POST
          path: /avatar/key
          body:
            note: "{note}"
            pressed: "{value}"
```

---

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
