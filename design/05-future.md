# 未解決・将来の検討事項

## 将来要件（初期スコープ外）

初期実装には含めないが、設計上の拡張ポイントとして念頭に置く要件。

### 入出力ドライバーの双方向化

初期は「MIDI 入力 → OSC 出力」のみだが、プロトコルはドライバーの差し替えで入出力どちらにも使えることを想定する。

| ドライバー | 入力 | 出力 |
|---|---|---|
| MIDI | ✅ 初期実装 | 将来対応（e.g. MIDI クロック送信、フィードバック） |
| OSC | 将来対応（e.g. VRChat からの状態受信） | ✅ 初期実装 |
| BLE Heart Rate | 将来対応（`direction: input`） | — |
| WebSocket | 将来対応 | 将来対応 |
| HTTP | 将来対応 | 将来対応 |

#### HTTP ドライバーのイメージ

HTTP はドライバー固有の I/O モデルを持つ：

**入力（サーバー起動型）**：ブリッジ起動時に HTTP サーバーが指定ポートで立ち上がる。
Device Profile の `definition` は受け付ける API エンドポイントを記述し、`binding` でリクエストボディのフィールドを ComponentState にマッピングする。

```yaml
# 入力 Device Profile（driver: http）のイメージ
definition:
  components:
    - id: note_trigger
      type: pulse
    - id: expression
      type: slider
      range: 0~1

binding:
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

**出力（HTTP クライアント型）**：Signal が発生するたびに Preferences で設定した URL へ JSON body をリクエスト送出する。

```yaml
# 出力 Device Profile（driver: http）のイメージ
binding:
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

### 複数入出力の同時設定

初期は入力 1 系統・出力 1 系統の構成のみだが、将来は複数の入力・出力を同時に設定し、1つのマッパーでまとめてルーティングできる構成を目指す。

```
入力ドライバー A ─┐
入力ドライバー B ─┤→ Mapper → 出力ドライバー X
入力ドライバー C ─┘          出力ドライバー Y
```

設定ファイルの `pipeline` に複数の `input_source` / `transport` を列挙できるよう拡張する。初期設計時から `driver` / `transport` をリスト構造にしておくことで移行コストを下げる。

### AI によるパイプライン自動構成

接続された入力デバイスと指定した出力ターゲットの情報をもとに、AI が Input Source Profile・Mapper・Output Target Profile を自動生成・提案する機能。

想定フロー：
1. ユーザーがデバイスを接続し、出力先（例: VRChat アバター）を指定する
2. AI が raw events のサンプリング結果とアバターパラメーター一覧を解析する
3. binding・マッピング・ルーティングの初期案を生成してユーザーに提示する
4. ユーザーが GUI で調整・確定する

---

## 未解決事項

| 項目 | 内容 |
|---|---|
| ELS-03 チャンネルマップ | 実機確認が必要。判明後 `els03.yaml` の binding に反映 |
| ELS-03 キー横傾きの MIDI 実装 | MPE / チャンネル PitchBend / SysEx のいずれかを実機確認で特定 |
| Mapper の複合ロジック | 和音検出・時系列処理は現時点で対応外。将来拡張ポイント |
| OSCQuery 対応 | VRChat 起動中にアバターパラメーターをリアルタイム取得。初期実装はローカルファイル読み取りで代替 |
| VRChat アバター config 参照 | `AppData/.../OSC/{userId}/Avatars/{avatarId}.json` をパースしてパラメーター補完に使う |
| 追加入力ドライバー | `ble-heart-rate`, `keyboard`, `osc-input` など |
| 追加出力ドライバー | `websocket`, `serial` など |

## 参考リンク

- [Yamaha ELS-03 MIDI リファレンス](https://jp.yamaha.com/) — 機種別 PDF（実機確認要）
- [VRChat OSC Avatar Parameters](https://docs.vrchat.com/docs/osc-avatar-parameters)
- [VRChat OSC Resources](https://docs.vrchat.com/docs/osc-resources)
- [VRChat OSCQuery](https://docs.vrchat.com/docs/oscquery)
- [midir（Rust MIDI ライブラリ）](https://github.com/Boddlnagg/midir)
- [rosc（Rust OSC ライブラリ）](https://github.com/klingtnet/rosc)
- [Tauri](https://tauri.app)
