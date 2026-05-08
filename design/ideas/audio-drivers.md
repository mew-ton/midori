# Audio 系ドライバーのイメージ

> ステータス：アイデアレベル
> 最終更新：2026-05-08

マイク入力から特徴量を抽出するドライバー群。初期スコープ外。`05-future.md` の「追加ドライバー」表に「将来」として列挙されているもののスケッチ。[ドライバー分割の粒度指標](../layers/01-input-driver/requirements.md#ドライバー分割の粒度指標) の具体適用例にもなる。

## ドライバー一覧

- `audio-spectrum`: 楽器・環境音向け。接続設定 = 入力デバイス選択 + `fft_size` / `band_count` / `window`。出力 = `static_array<float>`（長さ = band_count）
- `audio-voice`: ボイス特化。接続設定 = 入力デバイス選択 + `model_path` / `frame_ms` / `smoothing`。出力 =
  - viseme weights: `static_array<float>`（長さ = 15, [OVRLipSync](https://developers.meta.com/horizon/documentation/unity/audio-ovrlipsync-viseme-reference/) 準拠）
  - dominant viseme: `int`（0–14）
  - volume: `float`（range [0, 1]、RMS 由来の正規化値）

将来追加候補: `audio-music`（beat / chord / key）など。

## 粒度指標の当てはめ

| 比較 | 軸 1 時刻結合 | 軸 2 目的 | 軸 3 パラメーター系 | 軸 4 計算特性 | 結論 |
|---|---|---|---|---|---|
| viseme と volume | YES（リップシンクで位相一致が効く） | — | — | — | **同一 `audio-voice` に畳む**（軸 1 で確定） |
| `audio-voice` と `audio-spectrum` | NO（独立して解釈する） | NO（ボイス vs 楽器） | NO（model vs fft_size） | YES（ML 推論 vs 純 DSP） | **別ドライバー** |
| `audio-spectrum` と仮想 `audio-rms` | NO | やや YES（どちらも音量系の見方） | YES（同じ FFT パイプ） | NO（ともに軽量 DSP） | **別ドライバーにしない**（`audio-spectrum` の component に RMS を足す） |

## 命名

`<modality>-<purpose>` 規則に従い、`audio-` プレフィックスで並べる。手段命名（`audio-fft` / `audio-onnx-viseme` 等）は避ける。命名ルールの全文 → [ドライバー分割の粒度指標 § ネームスペース命名](../layers/01-input-driver/requirements.md#ネームスペース命名)

## 同一マイクを 2 ドライバーで共有する構成について

**起動時バリデーションエラーになる**（[物理入力の重複禁止](../layers/01-input-driver/requirements.md#物理入力の重複禁止)）。`audio` modality の `physical_input_identity: [device_name]` を Bridge が突き合わせ、2 つの inputs が同じデバイスを指している時点で profile load が失敗する。

仮にこの仕組みがなく許してしまった場合の問題（=禁止する根拠）：

- マイクの同時 open は OS 依存（macOS / Windows shared mode は OK、Linux ALSA 直叩きは不可）
- 各ドライバーが独立した内部バッファ・解析フレームを持つため**フレーム位相が揃わない**（数十 ms ズレる）
- 同じ PCM のデコードと窓掛けが二重化する

同一マイクから複数特徴量が必要な場合は **1 ドライバー多 component 構成**（粒度指標 軸 1）を取る。`audio-voice` が viseme + volume を 1 ドライバーで出すのはこの適用例。

なお同じ audio トランスポートに対して用途違いを `adapter_kind` で切り替える案は、アダプター種別定義がコードを持てない制約（[`../10-driver-plugin.md`](../10-driver-plugin.md)）により採用できない。

## 設計上の裏付け

FFT / ML 推論を Layer 1 に置く正当性は [`../layers/01-input-driver/requirements.md#コーデックの射程`](../layers/01-input-driver/requirements.md#コーデックの射程) を参照。

対応が必要になる周辺要素：
- 新しい component type（`spectrum` / `viseme` 等）または既存の `static_array<float>` / `slider` を組み合わせた component 表現
- 新しい mapper ノード（例: `argmax` — `static_array<float>` → `int`）
- ドライバーの permissions に `microphone` を追加（Phase 2 以降。[`../11-security/01-driver-sandbox.md`](../11-security/01-driver-sandbox.md)）
- `device-select` の `list` サブコマンドが OS の音声入力デバイス列挙にも対応すること（現仕様の範囲内）

## 関連

- レイテンシ的に tick 側で吸収できるかという観察 → [`./tick-vs-waveform.md`](./tick-vs-waveform.md)
