# Midori 設計全体レビュー（最終監査）

全体的なドキュメント群（`01`〜`10`、`config/`、`layers/`）の通読・監査を行いました。
直近で大きなアーキテクチャ変更（ドライバーの外部プロセス化、`device_kind` の分離、共有メモリによる通信、AI連携強化など）が行われましたが、**各ドキュメント間でこれらの概念が首尾一貫して定義されており、矛盾や破綻は見られません。** 
実装フェーズに移行するための基盤として十分に洗練されていると評価できます。

以下に、主要な観点ごとのレビュー結果と、実装フェーズにおいて留意すべき「微小な技術的懸念点」をまとめます。

---

## 1. アーキテクチャと概念の整合性評価

### ✅ ドライバーと `device_kind` の分離
`10-driver-plugin.md` と `config/02-device-config.md` の間で非常に美しく整理されています。
ドライバー（物理 I/O トランスポート）と `device_kind`（デバイス種別定義・正規化ルールのマニフェスト）が明確に分離されたことで、`osc-vrchat` などの「同じ通信プロトコルだが特別な付加情報や挙動を持つもの」を、コードを増やさずに宣言的に扱えるようになりました。プロファイル（`05-profile.md`）の接続フォーム構築メカニズムとも完全に噛み合っています。

### ✅ Bridge ↔ Driver のプロセス間通信
`10-driver-plugin.md` で「制御/ログは stdout/stdin」「高頻度のリアルタイムイベントは共有メモリ」と使い分けが明記されたことで、MIDI に求められる低レイテンシ（< 1ms）と、CLI アプリとしての扱いやすさ（JSON Lines での制御）が両立できる構造になりました。

### ✅ エラー伝播と UI 可視化
`06-error-handling.md` で定義された「クリティカルエラー（起動不可）」と「ランタイムエラー（経路の赤表示）」の分離が、5層パイプラインの思想と合致しています。エラーが起きた経路（`error-path`）を `Signal` ごとに追跡し、GUI（`04-runtime-cli.md`）で `data-error="1"` として赤く表示するデータフローは論理的に破綻していません。

### ✅ AI アシスタントとの統合
`08-ai.md` での「コンテキストのレイヤー分け（Skills / ドライバー知識 / デバイス spec）」と、`metadata.spec` による仕様書の埋め込みは、AI に過剰なトークンを消費させずに正確な YAML を生成させる優れたアプローチです。プロンプトインジェクション対策（`<external_data>`）も考慮されており、セキュアな基盤となっています。

---

## 2. 潜在的な技術的懸念事項（実装時の留意点）

設計としては矛盾はありませんが、実装時に課題になる可能性があるポイントです。

### 🚨 懸念1: Bridge ↔ GUI 間のモニタリングデータの帯域
- **現状の設計 (`04-runtime-cli.md`)**: Bridge は Layer 2 / Layer 4 を通過した `device-state` を JSON Lines で stdout に流し、Electron (Astro SSR) がそれをパースして SSE (`/events`) で GUI に転送します。
- **懸念**: MIDI デバイスでのエクスプレッションペダルの操作や、多数のキー同時押しなどが発生すると、1秒間に数百〜数千のイベントが発生する可能性があります。これをすべて JSON にエンコードして stdout に書き込み、Node.js がパースして SSE で流す過程で、**CPU 負荷の増大やバッファ詰まり**が発生する懸念があります。
- **実装時の推奨策**: 
  1. Bridge 側で GUI 向けに出力する `device-state` イベントは、一定レート（例: 60fps / 約16ms間隔）でデシメーション（間引き・最新値のサンプリング）して流す仕組みを入れる。
  2. あるいは、CLI 引数で `--monitor-rate=60` のように制限をかけられるようにする。

### 🚨 懸念2: `mirror` 機能の逆関数解決の複雑性
- **現状の設計 (`config/02-device-config.md`)**: `mirror` 記述により、`binding.input` の逆写像を自動で `binding.output` 用に生成します（全単射のチェックを実施）。
- **懸念**: `setMap.map` などでの重複チェックや、「noteOn/noteOff ペア」の推論は、ロジックとしてエッジケースを踏み抜きやすい部分です。
- **実装時の推奨策**: Rust での実装時、`mirror` の解決ロジック専用に十分なユニットテスト（特にエラーにすべき多対一のマッピングケース）を設ける必要があります。

### 🚨 懸念3: `octave_offset` と `key_range` の解釈の衝突
- **現状の設計 (`config/02-device-config.md`)**: `octave_offset`（例: -1）を指定すると、`c3` と書いたときに内部 note が `60` として解釈されます。
- **懸念**: GUI エディタ側でユーザーが「音名」で入力するか「ノート番号」で入力するかの UX がブレると、設定ファイル上のテキストと実際の MIDI 番号が意図せずズレる可能性があります。
- **実装時の推奨策**: GUI の Definition Editor では、「ユーザーが入力した文字（Yamaha記法）」と「最終的に計算される内部 note 番号（60など）」を常に並記してプレビュー表示する UI にすることが望ましいです。

---

## 3. 次のステップ

ドキュメント全体は極めて高い完成度となっており、設計段階での「手戻りを発生させるような致命的な論理破綻」はありません。
このまま **Phase 1（コアランタイムと基底 CLI の実装）** に進むことができる状態です。

今後の作業としては以下が考えられます：
1. **Runtime (Rust) のリポジトリ/クレート初期化** 
2. `midori-driver-sdk` の基礎設計（共有メモリの構造定義など）とモックドライバーの実装
3. パイプラインのイベント構造体（`raw-event`, `ComponentState`, `Signal`）の型定義の実装

設計の整合性に関しては完璧に整っているため、実装フェーズへ安心して移行していただけます。

---

## 2026-04-23 Round 1

### [design/12-distribution.md / design/12-ecosystem-readiness.md] ファイル番号の重複 ✅ 解決済み

`13-ecosystem-readiness.md` に改番し、README の参照も更新した。

### [profiles/devices/vrchat-osc/vrchat-osc.yaml:41] direction: any と受信専用パラメーターの整合性

`vrchat-osc.yaml` は `direction: any` で宣言されているが、`scene_index` コンポーネントは spec 内で「受信専用（VRChat → ブリッジ）」と明記されており、`binding.output` のマッピングにも `scene_index` は含まれていない（コメント: 受信専用のため output に含めない）。

`direction: any` のまま特定コンポーネントを出力側で使わない設計は、スキーマ上はバリデーションが通るが意図が不明瞭になる。

Option A: `direction: any` のまま運用し、`scene_index` の受信専用性はコメントと spec で伝える（現状維持）。
Option B: `direction: input` にしてデバイス構成を受信専用として宣言し、出力用（鍵盤・エクスプレッション・スライダー送信）は別ファイルに分離する。
Option C: 将来の設計として「component レベルの direction 制約」を追加する（現在の仕様にはない）。

### [design/07-ui-ux/07-profile.md:99,109 / design/06-error-handling.md:46] UIモックとログ例のデバイスパスがフラット形式

`07-profile.md` のプロファイル設定タブのUIモックでは `devices/yamaha-els03.yaml`（フラット形式）を使用している。`06-error-handling.md` のwarningログ例も同様。一方、`02-architecture.md` のリポジトリ構成図・実際の `profiles/devices/` サンプルはサブディレクトリ形式（`devices/yamaha-els03/yamaha-els03.yaml`）を示している。

Option A: UIモック・ログ例もサブディレクトリ形式に統一する。
Option B: UIは画面幅の都合上フラット表示を維持し、パスの形式はどちらでも動作することを注記として追加する（設計として強制しない）。

### [design/04-runtime-cli.md:76 / design/06-error-handling.md:30] ログの layer 名が統一されていない

`04-runtime-cli.md` の log イベント例では `"layer":"device-profile/input"` を使用しているが、`06-error-handling.md` のログ出力例では `"layer":"mapper"` / `"layer":"driver/osc"` が使われており、`device-profile/input` という文字列形式が規約として明文化されていない。

Option A: `04-runtime-cli.md` のログ例にある `device-profile/input` を正式な layer 識別子として `config/` または `00-naming.md` に定義する。
Option B: ログの layer 文字列は実装の自由とし、ドキュメント内の例示は例示として扱い規約化しない。
