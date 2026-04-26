# 未解決・将来の検討事項

## 将来要件（初期スコープ外）

初期実装には含めないが、設計上の拡張ポイントとして念頭に置く要件。

### 追加ドライバー

初期実装では MIDI / OSC を双方向でサポートする。VRChat 用の OSC 設定は `osc-vrchat` アダプター種別定義 として提供する。追加ドライバーは将来拡張。

| ドライバー | 入力 | 出力 |
|---|---|---|
| MIDI | ✅ 初期実装 | ✅ 初期実装 |
| OSC（`osc`） | ✅ 初期実装 | ✅ 初期実装 |
| BLE Heart Rate | 将来 | — |
| WebSocket | 将来 | 将来 |
| HTTP | 将来 | 将来 |
| Audio Spectrum | 将来 | — |
| Audio Voice | 将来 | — |

`osc-vrchat` は独立ドライバーではなく、`osc` ドライバーを基底とする **アダプター種別定義** として提供する。詳細 → [`10-driver-plugin.md`](10-driver-plugin.md)

#### HTTP ドライバーのイメージ

HTTP はドライバー固有の I/O モデルを持つ：

**入力（サーバー起動型）**：ブリッジ起動時に HTTP サーバーが指定ポートで立ち上がる。
アダプター の `definition` は受け付ける API エンドポイントを記述し、`binding` でリクエストボディのフィールドを ComponentState にマッピングする。

```yaml
# 入力 アダプター（driver: http）のイメージ
definition:
  components:
    - id: note_trigger
      type: pulser
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
          set: pulse
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
# 出力 アダプター（driver: http）のイメージ
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

**起動時バリデーションエラーになる**（[物理入力の重複禁止](layers/01-input-driver/requirements.md#物理入力の重複禁止)）。`audio` modality の `physical_input_identity: [device_name]` を Bridge が突き合わせ、2 つの inputs が同じデバイスを指している時点で profile load が失敗する。

仮にこの仕組みがなく許してしまった場合の問題（=禁止する根拠）：

- マイクの同時 open は OS 依存（macOS / Windows shared mode は OK、Linux ALSA 直叩きは不可）
- 各ドライバーが独立した内部バッファ・解析フレームを持つため**フレーム位相が揃わない**（数十 ms ズレる）
- 同じ PCM のデコードと窓掛けが二重化する

同一マイクから複数特徴量が必要な場合は **1 ドライバー多 component 構成**（粒度指標 軸 1）を取る。`audio-voice` が viseme + volume を 1 ドライバーで出すのはこの適用例。

なお同じ audio トランスポートに対して用途違いを `adapter_kind` で切り替える案は、アダプター種別定義がコードを持てない制約（[10-driver-plugin.md](10-driver-plugin.md)）により採用できない。

##### 設計上の裏付け

FFT / ML 推論を Layer 1 に置く正当性は [01-input-driver/requirements.md#コーデックの射程](layers/01-input-driver/requirements.md#コーデックの射程) を参照。

対応が必要になる周辺要素：
- 新しい component type（`spectrum` / `viseme` 等）または既存の `static_array<float>` / `slider` を組み合わせた component 表現
- 新しい mapper ノード（例: `argmax` — `static_array<float>` → `int`）
- ドライバーの permissions に `microphone` を追加（Phase 2 以降。[`11-security/01-driver-sandbox.md`](11-security/01-driver-sandbox.md)）
- `device-select` の `list` サブコマンドが OS の音声入力デバイス列挙にも対応すること（現仕様の範囲内）

### `char` primitive

文字データを扱うための primitive。配信制作向けアダプター（obs-websocket / vtube-studio / twitch-eventsub / spotify-now-playing / pixoo / 等）で必要になる。初期スコープには含めず、本節の指針に従って後段で追加する。

格納コストの一般則は [config/syntax/03-storage-model.md](config/syntax/03-storage-model.md) を参照。

#### char の単位

`char` は **NFC 正規化済みの 1 grapheme cluster** を表す opaque 型として定義する。

- 算術演算（加減算・bit 演算）は持たない
- 利用可能な操作は (a) `char[]` 全体を文字列処理ノード（concat / format / regex / equals / hash / take / 等）に渡す、(b) N 番目の要素を抽出して `char` として取り出す、の 2 種に限る
- truncate / length / indexing が「人間が見る 1 文字」の単位で動くことを保証する（絵文字・結合文字・国旗・肌色修飾子で破綻しない）

#### 格納モデル

- 内部表現は **NFC UTF-8 バイト列 + grapheme 境界インデックス**
- 1 grapheme の最大バイト幅は `MAX_GRAPHEME_BYTES`（既定 64）で打ち切る。超過時は driver / adapter 入口で強制境界分割する
- `static_array<char, N>` の物理メモリ予算は `N * MAX_GRAPHEME_BYTES`
- ASCII 確定フィールド向けに `char[N, ascii]` 修飾で `MAX_GRAPHEME_BYTES = 1` を強制する選択肢を持つ

`char` を opaque に保つ限り「1 要素のバイト幅が可変」であっても [config/syntax/03-storage-model.md](config/syntax/03-storage-model.md) の二層モデルは崩れない。算術ノードに `char` を流通させない、という制約だけを守る。

#### NFC 正規化

driver / adapter で受信した文字データは **入口で 1 回だけ NFC 正規化を行ってからマッパー層へ流す**。マッパー内部に流通する文字データは常に NFC 形と仮定でき、equals / hash / regex の比較が char-by-char で安定する。

採用する **Unicode バージョン** と **正規化実装ライブラリ** は char 実装 PR の時点で固定し、ランタイムの定数・設定として記録する。Unicode バージョンが上がった際は、grapheme segmentation 規則 (`MAX_GRAPHEME_BYTES` の妥当性含む) と正規化結果の差分が profile / adapter の互換性に与える影響を明示してから上げる。

#### 文字データの境界規律

文字データの長さに関する規律は **境界（driver / 出力ポート）でだけ強制し、マッパー内部は自由**とする:

| 経路 | 許可される型 |
|---|---|
| driver / adapter からマッパーへの流入 | `static_array<char, N>` のみ。`N` は adapter kind が宣言する |
| マッパーグラフ内部 | `static_array<char, N>` および `dynamic_array<char>` の両方を流通可 |
| 出力ブロックのポート | `static_array<char, N>` のみ。`dynamic_array<char>` の到達時はオーバーフローポリシで bounded バッファに書き写す |

`dynamic_array<char>` は実装上、入力長から上限が静的に算出される bounded scratch として確保され、steady state ではアロケーションを行わない（[03-storage-model.md](config/syntax/03-storage-model.md) の `dynamic_array<T>` 一般則の char への適用）。

#### オーバーフローポリシ

`on_overflow` フィールドで指定する。`out_of_range`（数値版）の char 版に相当する:

| 値 | 挙動 | 用途 |
|---|---|---|
| `truncate` | USV 境界で切る | 内部処理・ログ・ASCII 想定フィールド |
| `truncate_grapheme` | grapheme cluster 境界で切る（N USV を超えない最大長） | 表示系（Pixoo / OBS テキスト / Stream Deck ボタンラベル 等） |
| `drop_event` | イベント自体を破棄 | 不正データ排除 |
| `error` | バリデーションエラー / イベント skip | UUID 等の長さが正確である必要があるフィールド |

表示系アダプターは `truncate_grapheme` を既定とする。

#### 例外: 長さ無制限の入力

「いくらでも長くしてよい何らかの機能から文字列をもらう」ケースは例外として扱う。アダプター kind が明示的に `unbounded_text` capability を宣言した場合のみ許可し、当該アダプターは出力エンドポイントとしてのみ接続できる（マッパー通常 signal としては流通させない）。TTS のような端点用途を想定する。

#### 関連ノード（char と同時に追加）

- 生成: `format` / `concat`
- 分解: `split` / `substring` / `regex_capture`
- 検査: `equals` / `regex_match` / `starts_with` / `contains`
- 変換: `parse_int` / `parse_float` / `to_string` / `to_lower` / `to_upper` / `trim`
- ディスパッチ: `setMap.map` の char キー対応

#### 導入時のチェックリスト

[03-storage-model.md § 新規 primitive 追加のコスト](config/syntax/03-storage-model.md#新規-primitive-追加のコスト) に従い、以下を揃えてから initial scope に取り込む:

1. UTF-8 + grapheme 境界インデックスの格納実装
2. Unicode 正規化（NFC）と grapheme segmentation の依存追加
3. **採用 Unicode バージョンと正規化／segmentation ライブラリの固定**（ランタイム定数として記録。アップグレード手順を運用ドキュメントに併記）
4. driver / adapter 入口の NFC 正規化処理
5. 上記関連ノードの specialized 実装
6. アダプター kind スキーマの `char[N]` / `on_overflow` 宣言サポート
7. [config/syntax/02-value-types.md](config/syntax/02-value-types.md) の primitive 表更新

#### 適用ユースケース（動機）

- OBS / vMix のシーン名・ソース名による指定
- VTube Studio のホットキー UUID 指定
- Twitch チャット / Channel Points / Redeem の文字列入力
- Now Playing 系（Spotify 等）の楽曲・アーティスト名表示
- Pixoo / Divoom 等の物理ディスプレイへのテキスト送出
- SysEx パッチ名・ソング名の表示連携

これらのアダプターは [09-plugin.md](09-plugin.md) のプラグイン枠で順次追加する。

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
