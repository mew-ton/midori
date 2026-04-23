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

マイク入力から特徴量を抽出するドライバー群。[ドライバー分割の粒度指標](layers/01-input-driver/requirements.md#ドライバー分割の粒度指標) の具体適用例。

##### ドライバー一覧

- `audio-spectrum`: 楽器・環境音向け。接続設定 = 入力デバイス選択 + `fft_size` / `band_count` / `window`。出力 = `static_array<float>`（長さ = band_count）
- `audio-voice`: ボイス特化。接続設定 = 入力デバイス選択 + `model_path` / `frame_ms` / `smoothing`。出力 =
  - viseme weights: `static_array<float>`（長さ = 15, [OVRLipSync](https://developers.meta.com/horizon/documentation/unity/audio-ovrlipsync-viseme-reference/) 準拠）
  - dominant viseme: `int`（0–14）
  - volume: `float`（range [0, 1]、RMS 由来の正規化値）

将来追加候補: `audio-music`（beat / chord / key）など。

##### 粒度指標の当てはめ

| 比較 | 軸 1 時刻結合 | 軸 2 目的 | 軸 3 パラメーター系 | 軸 4 計算特性 | 結論 |
|---|---|---|---|---|---|
| viseme と volume | YES（リップシンクで位相一致が効く） | — | — | — | **同一 `audio-voice` に畳む**（軸 1 で確定） |
| `audio-voice` と `audio-spectrum` | NO（独立して解釈する） | NO（ボイス vs 楽器） | NO（model vs fft_size） | YES（ML 推論 vs 純 DSP） | **別ドライバー** |
| `audio-spectrum` と仮想 `audio-rms` | NO | やや YES（どちらも音量系の見方） | YES（同じ FFT パイプ） | NO（ともに軽量 DSP） | **別ドライバーにしない**（`audio-spectrum` の component に RMS を足す） |

##### 命名

`<modality>-<purpose>` 規則に従い、`audio-` プレフィックスで並べる。手段命名（`audio-fft` / `audio-onnx-viseme` 等）は避ける。命名ルールの全文 → [ドライバー分割の粒度指標 § ネームスペース命名](layers/01-input-driver/requirements.md#ネームスペース命名)

##### 同一マイクを 2 ドライバーで共有する構成について

原則として推奨しない。粒度指標の軸 1（時刻結合）が YES の特徴量を別ドライバーに分けてしまった場合に発生する事態であり、その時点で設計が間違っている。技術的にも以下の問題がある：

- マイクの同時 open は OS 依存（macOS / Windows shared mode は OK、Linux ALSA 直叩きは不可）
- 各ドライバーが独立した内部バッファ・解析フレームを持つため**フレーム位相が揃わない**（数十 ms ズレる）
- 同じ PCM のデコードと窓掛けが二重化する

なお同じ audio トランスポートに対して用途違いを `device_kind` で切り替える案は、デバイス種別定義がコードを持てない制約（[10-driver-plugin.md](10-driver-plugin.md)）により採用できない。

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
