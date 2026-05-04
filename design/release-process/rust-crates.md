# Rust Crates Release Process

> ステータス：運用ドキュメント
> 最終更新：2026-05-05

このドキュメントは midori プロジェクトの release 運用のうち **Rust crates（`midori-core`, `midori-sdk`）の crates.io への publish** を扱う。Electron デスクトップアプリのバイナリ配布や JS パッケージの npm 公開は別トラックで、各 sibling ドキュメント（`./README.md` の一覧参照）が独立して扱う。エンドユーザー向けアプリ配布の方針は `../12-distribution.md`。

公開対象クレートと公開先方針の決定は `../14-repository-structure.md` が一次ソース。本ドキュメントは release 作業手順の前提として要点を再掲しつつ、「**いつ・どうやって publish するか**」の手順を定める。

---

## 公開対象クレート

| クレート | 公開先 | 理由 |
|---|---|---|
| `midori-core` | crates.io | 型・プロトコル定義。`midori-sdk` が依存する |
| `midori-sdk` | crates.io | ドライバー作者が直接依存するライブラリ |
| `midori-runtime` | 公開しない | バイナリ配布（`../12-distribution.md`）と npm shim 経由 |
| `midori-driver-midi` / `midori-driver-osc` | 公開しない | 公式ドライバーバイナリ。`midori-runtime` に同梱 |
| `midori-driver-dummy` | 公開しない | runtime の lifecycle / handshake 統合テスト用ハーネス。本番 runtime には含めない |
| `midori-ipc-shm` | 公開しない | workspace 内部 crate（`unsafe` 隔離目的） |

publish 対象 crate には `Cargo.toml` の `description` / `license` / `repository` / `readme` が揃っている必要がある。`midori-ipc-shm` のような workspace 内部専用 crate はこれらの整備義務を負わない。

---

## バージョニング戦略

### Semver 適用境界

| 区分 | 適用条件 |
|---|---|
| major bump (`X.0.0`) | wire / ABI / public API の **後方互換のない変更** |
| minor bump (`x.Y.0`) | public API への **後方互換のある追加** |
| patch bump (`x.y.Z`) | バグ修正・内部リファクタのみ。public API・ABI 双方に影響しない |

各クレートは **独立にバージョン管理**する。workspace 統一バージョンは採用しない。`midori-core` と `midori-sdk` は依存関係上連動して bump されることが多いが、bump 規模は別個に判断する。

### major bump の判断基準

以下のいずれかに該当する変更は major bump を要する：

- `#[repr(C)]` 構造体のレイアウト変更（フィールドの追加・削除・並び替え・型変更）
- C FFI シグネチャの変更（引数追加 / 戻り値型変更 / 関数削除）
- `cbindgen` で生成される C ヘッダの ABI 影響変更
- `pub` 構造体のフィールド削除・型変更
- `pub` 関数のシグネチャ変更（引数追加・型変更を含む）
- `pub` re-export の削除
- 公開 trait への **デフォルト実装なし** メソッド追加（既存 implementor に新メソッドの実装を強制するため）。デフォルト実装ありの追加は後方互換 = minor

迷う場合は major bump を選ぶ。crates.io は publish 後の version 削除をサポートしないため、互換性を弱く宣言してしまう方がリスクが大きい。

例外として `../15-sdk-bindings-api.md` で確立済みのパターン（`struct_size` guard を持つ `#[repr(C)]` 構造体への末尾追加、`#[non_exhaustive]` enum へのバリアント追加）は **minor bump で扱える**。これらは旧バイナリ / 旧 enum match を壊さないことが構造的に保証されているため。詳細な適用条件は同ドキュメントの該当節に従う。

### `Cargo.lock` と `[workspace.dependencies].version`

workspace dependency entry は **公開する / しないに関わらず** `path` と `version` の両方を持たせる。`cargo-deny` の wildcard ゲートは publish 対象だけを見るのではなく workspace 全体の依存グラフを評価するため、内部 crate も例外にできない:

```toml
[workspace.dependencies]
midori-core    = { path = "crates/midori-core",    version = "0.3.0" }
midori-sdk     = { path = "crates/midori-sdk",     version = "0.2.0" }
midori-ipc-shm = { path = "crates/midori-ipc-shm", version = "0.1.0" }
midori-runtime = { path = "crates/midori-runtime", version = "0.1.0" }
```

`version` を欠いた path-only エントリは `cargo publish` 時に wildcard 扱いとなり、`cargo-deny` の `bans.wildcards = "deny"` ゲートで失敗する。bump のたびに該当 crate の `version` も更新する。

---

## タグ命名規則

`<crate-name>-vX.Y.Z` 形式（例: `midori-core-v0.3.0`, `midori-sdk-v0.2.0`）。

| 採用しない理由 | |
|---|---|
| `vX.Y.Z` 統一タグ | 複数 crate を独立に bump するため、どの crate の release か判別できない |
| `release/midori-core/0.3.0` 等のスラッシュ区切り | `git tag` の listing と GitHub Release の sort で扱いづらい |

タグは publish 完了 commit に打つ。`cargo publish` が成功してから tag を push する順序を守る（publish 失敗時の retry をクリーンに保つため）。

---

## CHANGELOG 運用

各 publish 対象 crate は `CHANGELOG.md` を持ち、以降は本ルール（[Keep a Changelog](https://keepachangelog.com/) 準拠）で版管理する。`midori-sdk` は現時点で `CHANGELOG.md` を持たないため、初回 publish の前提（後述）として作成し、その後は他の crate と同様に運用する：

```text
crates/midori-core/CHANGELOG.md
crates/midori-sdk/CHANGELOG.md
```

形式は次の通り：

```markdown
# Changelog — <crate-name>

## X.Y.Z — YYYY-MM-DD

### Breaking changes

- 破壊変更の説明（major bump 時のみセクションを起こす）

### Added

- 追加された public API / 機能

### Changed

- 既存挙動の変更（破壊しない範囲）

### Fixed

- バグ修正
```

major bump がない release では `### Breaking changes` セクションは省略する。`### Added` / `### Changed` / `### Fixed` も該当変更がない場合は省略する。

---

## 手動 publish 手順

GitHub Actions workflow が整うまで、または ad-hoc release では本手順を踏む。

### 前提

- `cargo login <token>` 済み（`crates.io` で発行した API token）
- 対象 crate の `Cargo.toml` の `version` を bump 済み
- 対象 crate の `CHANGELOG.md` に該当 version のエントリを追加済み
- workspace ルートの `[workspace.dependencies]` にある対象 crate の `version` も bump 済み（依存側の crate が壊れないこと）
- main ブランチにマージ済み（publish の起点 commit が main から外れていないこと）

### 実行

依存順序に注意する。`midori-sdk` は `midori-core` に依存するため、`midori-core` を先に publish して crates.io 側で resolvable になってから `midori-sdk` を publish する。

```bash
# 1. dry-run で midori-core の package をローカル検証（package build / lint / 依存解決を通すが publish はしない）
cargo publish --dry-run -p midori-core

# 2. dry-run が green なら本 publish
cargo publish -p midori-core

# 3. crates.io 側で index 反映を待つ（数秒〜数十秒）
#    反映されないうちに依存 crate を publish するとエラーになる

# 4. midori-sdk の dry-run。`cargo publish --dry-run` は package 化のとき
#    `path` 依存を stripped し、`version` 要件のみを crates.io index に対して
#    解決する（path は registry に存在しない / 公開できないため）。よって本 step
#    は「公開済 midori-core が crates.io から resolvable か」の検証として機能する。
#    `index 反映遅延` のために手順 3 の wait が短すぎるとここで失敗する → retry
cargo publish --dry-run -p midori-sdk

# 5. midori-sdk を publish（手順 4 と同じ resolution path で本適用）
cargo publish -p midori-sdk

# 6. 両 publish 成功後に tag を打って push（手順 5 まで通ってから）
#    <publish-commit> は通常 HEAD（main にマージ済みの release 起点 commit）。
#    過去の commit を release する場合は対応する commit hash を指定する
git tag midori-core-v0.3.0 <publish-commit>
git tag midori-sdk-v0.2.0 <publish-commit>
git push origin midori-core-v0.3.0 midori-sdk-v0.2.0
```

### 失敗時の挙動

- `cargo publish` が途中で失敗した場合、既に publish 済みの crate は yank できるが完全削除はできない。version を捨てて次の patch / minor へ進めるのが基本対応
- 手順 4 の `--dry-run` が "no matching package named `midori-core`" 等で失敗する場合は、手順 3 の index 反映待ちが不足している。時間を置いて retry すれば通る
- 一度 publish した version の中身は変更不可。修正が必要なら次の version を切る

---

## GitHub Actions release workflow

`.github/workflows/release.yml` を canonical な publish 経路として維持する。本ファイルが repo 内に存在しない場合は、本セクションの規約（トリガー / Secrets / 推奨デフォルト）に従って新規追加する。

### トリガー

- `<crate>-vX.Y.Z` パターンの tag push（main へマージ後の自然な release flow）
- もしくは `workflow_dispatch`（crate 名と version を input、dry-run flag 付き）

### Secrets

- `secrets.CRATES_IO_TOKEN` を repository secret として登録する（メンテナの `cargo login` token）

### 推奨デフォルト

- `workflow_dispatch` の `dry-run` input を **デフォルト true** にし、本 publish は明示的に false にしてから実行する
- workflow の permissions は最小（`contents: read` を基本、Release 作成連携が要るなら `contents: write`）
- publish 失敗ログは job の output で確認できるようにし、retry 判断の材料を残す

手動手順は workflow が落ちたとき / ad-hoc release のための fallback として常に維持する。

---

## バージョニング詳細ルール（保留）

semver 適用境界の細かい運用ルール（例: deprecation 期間、`#[non_exhaustive]` 適用ガイドラインの拡張、breaking 範囲のグレーゾーン判定）は本ドキュメントに含めない。

`../15-sdk-bindings-api.md` は **特定の拡張パターン**（`struct_size` guard を持つ `#[repr(C)]` 構造体への末尾追加、`#[non_exhaustive]` enum へのバリアント追加）を minor bump で扱える根拠として確立しているが、semver 境界線そのものの確定は同ドキュメントの「スコープに入れないもの」に明記されている。本ドキュメントの上記「major bump の判断基準」が運用上の境界の現時点での合意。運用上のオーバーラップが顕在化した時点で別ドキュメントに切り出す。

現状の運用粒度: 「迷ったら major bump」「破壊変更は CHANGELOG の `### Breaking changes` で必ず予告する」だけで十分機能する。
