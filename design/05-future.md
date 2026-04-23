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
| Audio Spectrum | 将来 | — |
| Audio Voice | 将来 | — |

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

#### Audio 系ドライバーのイメージ

マイク入力から特徴量を抽出するドライバー。**用途別に専門化したドライバーを並べる**ことで、異なるマイクに異なる解析を掛けたいケース（例: エレクトーン集音マイク → 楽器スペクトラム、歌声用マイク → ボイス解析）を自然に表現する。

- `audio-spectrum`: 楽器・環境音向け。接続設定 = 入力デバイス選択 + `fft_size` / `band_count` / `window`。出力 = `static_array<float>`（長さ = band_count）
- `audio-voice`: ボイス特化。接続設定 = 入力デバイス選択 + `model_path` / `frame_ms` / `smoothing`。出力 =
  - viseme weights: `static_array<float>`（長さ = 15, [OVRLipSync](https://developers.meta.com/horizon/documentation/unity/audio-ovrlipsync-viseme-reference/) 準拠）
  - dominant viseme: `int`（0–14）
  - volume: `float`（range [0, 1]、RMS 由来の正規化値）

##### ドライバーを「用途別」に切る理由

ボイス入力は「viseme と volume を同じ瞬間の音声から取る」ことに意味がある（リップシンクと表情の振幅が時刻一致する）。同じマイクを `audio-viseme` と `audio-volume` の 2 ドライバーで共有する構成も理屈の上では可能だが、

- マイクの同時 open は OS 依存（macOS / Windows shared mode は OK、Linux ALSA 直叩きは不可）
- 各ドライバーが独立した内部バッファ・解析フレームを持つため**フレーム位相が揃わない**（数十 ms ズレる）
- 同じ PCM のデコードと窓掛けが二重化する

といった問題がある。**「同一の物理入力から得る関連特徴量は 1 ドライバーにまとめて多 component で出す」** 方が現行モデルと整合する。`audio-voice` がボイス用途で必要になる特徴量（viseme + volume + 将来的に pitch / energy 等）を一括で出すのはこの方針の適用例。

逆に「ボイス用と楽器用は別ドライバー」となるのは、扱う特徴量と適性パラメーター（fft_size / model 等）が用途によって大きく異なるため、単一ドライバーに畳むメリットが薄いから。

同じ audio トランスポートに対して用途違いを `device_kind` で切り替える案は、デバイス種別定義がコードを持てない制約（[10-driver-plugin.md](10-driver-plugin.md)）により採用できない。

##### 設計上の裏付け

FFT / ML 推論を Layer 1 に置く正当性は [01-input-driver/requirements.md#コーデックの射程](layers/01-input-driver/requirements.md#コーデックの射程) を参照。

対応が必要になる周辺要素：
- 新しい component type（`spectrum` / `viseme` 等）または既存の `static_array<float>` / `slider` を組み合わせた component 表現
- 新しい mapper ノード（例: `argmax` — `static_array<float>` → `int`）
- ドライバーの permissions に `microphone` を追加（Phase 2 以降。[`11-security/01-driver-sandbox.md`](11-security/01-driver-sandbox.md)）
- `device-select` の `list` サブコマンドが OS の音声入力デバイス列挙にも対応すること（現仕様の範囲内）

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
