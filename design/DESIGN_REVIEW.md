# 設計レビュー待ち項目

矛盾解消ループで「即修正」の判断基準を満たさなかった設計判断待ち項目。  
解決したら該当セクションを削除する。

---

## 2026-04-21 ラウンド 3

### [04-runtime-cli.md:70 / 02-architecture.md:25] signal イベントと device_id の整合

（ラウンド1から継続）詳細が確認できた。

**問題の詳細:**  
- `02-architecture.md` は Layer 3 出力を「Signal（device_id 付き）」と記載
- `04-runtime-cli.md` の signal イベント: `{"type":"signal","name":"upper.60.pressed","value":1.0}` — device_id なし
- `error-path` イベント（06-error-handling.md）の `"signals":["upper.60.velocity"]` も同様に device_id なし
- `config/03-mapper.md` のポート記法では `output.<device_id>.<Signal指定子>` と device_id を含む

**選択肢:**  
A: signal イベントに `"device"` フィールドを追加 `{"type":"signal","device":"vrchat-osc","name":"upper.60.pressed","value":1.0}`  
B: アーキテクチャ図の「device_id 付き」を修正し、Signal 指定子のみで識別すると統一。device_id との紐づけはルーティング時に行われ、イベントストリームには出現しない

---

### [08-ai.md:52] sample_raw_events ツールの呼び出しシグネチャ未定義

**問題:**  
`sample_raw_events` ツールが引数テーブルに登場するが呼び出しパラメータ（device_id, duration 等）と戻り値の形式が未記述。

**必要な仕様:**  
- パラメータ: `device_id: string`, `duration_ms?: number`（デフォルト値）
- 戻り値: raw events の配列（JSON Lines 形式 or 配列？）
- エラー時の動作（デバイス未接続等）

---

### [07-ui-ux/06-graph.md:113] 変換グラフテスターの状態遷移・デバイス接続設定

**問題:**  
変換グラフ編集画面のテスターモードは「ブリッジを内部的に起動」するが：
- 起動時にどのデバイス接続設定を使うか未定義（プロファイルがない状態）
- テスト実行中とプロファイル実行中の状態遷移が未定義
- テスト用の接続設定をどこに入力するか未定義

デバイス構成プレビュータブ（05-device-config.md）は「テスト接続設定」を preferences にキャッシュする仕様があるが、変換グラフテスターは2つのデバイスが必要で設定が複雑。

---

## 2026-04-21 ラウンド 2

### [config/02-device-config.md:406] mirror の setMap.map 全単射判定が未定義

**問題:**  
`mirror` の使用条件として「`setMap.map` は原則不可。全単射の map のみ例外的に可」と書かれているが、Bridge がその map が全単射かどうかを**どうやって判定するか**が未記述。

`setMap.map` の全単射判定は：
- すべての `set` 値が重複しない場合 = 全単射
- いずれかの `set` 値が重複する場合 = 非全単射 → mirror 不可

**選択肢:**  
A: Bridge が起動時に `setMap.map` の全エントリを走査し、出力値の重複チェックを行う。重複があればエラー  
B: `setMap.map` の `mirror` を単純に禁止し、仕様から「全単射なら可」の記述を削除する（実用上ほぼ存在しないため）

---

## 2026-04-21 ラウンド 1

### [02-architecture.md / 04-runtime-cli.md] Signal イベントに device_id が含まれない

**問題:**  
`02-architecture.md` のアーキテクチャ図では Layer 3 の出力を「Signal（device_id 付き）」と表記している。  
しかし `04-runtime-cli.md` の signal イベントは `{"type":"signal","name":"upper.60.pressed","value":1.0}` と device_id を含まない。

`design/config/03-mapper.md` のポート記法（`output.vrchat-default.upper.{note}.pressed`）では device_id を使うため、Signal にはどこかで device_id が紐づく必要がある。

**選択肢:**  
A: signal イベントに `"device"` フィールドを追加する（`device-state` イベントと対称にする）  
B: アーキテクチャ図の「device_id 付き」という表現を削除し、Signal は指定子のみで識別すると明記。device_id との紐づけは出力ルーティング時に行う

---

### [10-driver-plugin.md / 03-tech-stack.md] 公式ドライバーの「built-in なし」と自動プロビジョニングの説明矛盾

**問題:**  
`10-driver-plugin.md` は「すべてのドライバーはプラグインとして動作する。built-in（本体組み込み）という概念は持たない」と明記している。  
一方 `03-tech-stack.md` では「Electron 起動のたびに公式ドライバーのバージョンを確認し、未インストールの場合は自動インストールする」と説明している。

どちらも正しいが、ユーザー視点で「なぜインストール操作なしに MIDI が使えるのか」の説明が欠けている。

**選択肢:**  
A: `10-driver-plugin.md` または `09-plugin.md` に「公式ドライバーは Electron が初回起動時に自動インストールする。プラグインとして動作するがユーザーが意識する必要はない」という補足を追記する  
B: `01-overview.md` または README に「公式ドライバー自動セットアップ」の説明を追加する

---

### [05-device-config.md（UI） / 07-profile.md（UI）] デバッグ用ブリッジの多重起動と排他制御

**問題:**  
以下の3つの画面がそれぞれ独立してブリッジを起動できる：
- デバイス構成編集画面 → プレビュータブ（最小限ブリッジ）
- 変換グラフ編集画面 → テスターモード（テストブリッジ）
- プロファイル詳細画面 → 実行ボタン（本番ブリッジ）

未定義の事項：
1. 複数ブリッジの同時起動は許可するか
2. MIDI デバイスは1プロセスからしか開けないことが多い（OS 排他制御）。同一 MIDI デバイスを使う複数ブリッジの起動時どうするか
3. タイトルバーのブリッジ状態表示（`02-tone.md`）はどのブリッジの状態を示すか

**選択肢:**  
A: 同時起動を許可しない。起動試行時に「別のブリッジが動作中」とダイアログを出す  
B: 同時起動を許可する。タイトルバーには「最後に起動したブリッジ」または「最も重要なブリッジ（本番優先）」の状態を表示  
C: デバッグ用ブリッジは本番ブリッジとは別プロセス管理とし、デバイス別に排他を管理する

---

### [04-runtime-cli.md] connection_fields とプロファイル接続設定フィールドの対応関係

**問題:**  
`10-driver-plugin.md` の `connection_fields` でドライバーが接続設定フォームフィールドを宣言する仕様がある。  
`config/05-profile.md` の `connection` セクションには driver ごとのフィールド（`device_name`, `host`, `port` 等）が列挙されている。  
しかし「`connection_fields` 宣言 → GUI がフォーム自動生成 → プロファイルの `connection` に保存」という一連のフローが明文化されていない。

**選択肢:**  
A: `config/05-profile.md` に「`connection` フィールドはドライバーの `connection_fields` 宣言から自動生成される。フィールド ID が YAML のキーになる」という説明を追記する

---

### [config/03-mapper.md / profiles/] mapper ノード型の一覧ドキュメント未確認

**問題:**  
`profiles/mappers/els03-vrchat-simple.yaml` で使われているノード型（`array_merge`, `compact`, `take`, `flatten`, `defaults`, `to_bits`）が、`design/config/mapper-nodes/` 配下のファイルに定義されているか未確認。

実装時に仕様を参照できるよう確認が必要。  
**次のラウンドで `design/config/mapper-nodes/` を調査する。**
