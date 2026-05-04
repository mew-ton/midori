# Release Process

> ステータス：運用ドキュメント
> 最終更新：2026-05-05

このディレクトリは midori プロジェクトの **release MECHANICS**（成果物をどうやって公開するか）を扱う。track ごとに独立したサブドキュメントを置き、本 README はその一覧と横断的な考慮事項に絞る。

エンドユーザー視点の配布方針（配布窓口・自動アップデート・セキュリティマニフェスト・利用規約）は `../12-distribution.md` の責務。本ディレクトリと `12-distribution.md` の境界は次節で定義する。

---

## Track 一覧

| Track | 成果物 | 公開先 | ドキュメント | 状態 |
|---|---|---|---|---|
| Rust crates | `midori-core` / `midori-sdk` の crate | crates.io | [`rust-crates.md`](./rust-crates.md) | 確定 |
| Electron デスクトップアプリ | midori 本体バイナリ | 公式サイト / Booth | （未着手） | 計画 |
| JS パッケージ | `@midori/runtime` 系 npm shim、`@midori/ui` 等 | npm | （未着手） | 計画 |

各 track は **独立にリリースサイクルを持つ**。Rust crate の publish タイミング、Electron アプリの release 周期、npm パッケージの version 切り直しは互いに従属しない。横断的にバージョンを揃える運用ルールは現時点で導入しない（必要になった時点で本 README に追加する）。

未着手 track のドキュメントは、当該 track の最初の release 着手時に新規追加する。空ファイルや placeholder は作らない。

---

## このディレクトリと `12-distribution.md` の境界

両者を区別する単一の問い: **誰が読む文書か**。

| 観点 | `release-process/` | `12-distribution.md` |
|---|---|---|
| 読者 | メンテナ / contributor（成果物を**送り出す**側） | エンドユーザー / プラグイン作者（成果物を**受け取る**側） |
| 扱う事項 | publish コマンド、tag 規則、CHANGELOG 形式、CI workflow、token / secret 管理 | 配布窓口、自動アップデート方式、セキュリティマニフェスト、利用規約 |
| 不変の範囲 | 内部運用ルール（変更しても外向きには見えない） | ユーザーとの契約（変更時は移行手順が要る） |

例: 「`midori-core-v0.4.0` を crates.io に publish するときの dry-run 順序」は本ディレクトリ。「midori デスクトップアプリの 1.5.0 リリース時に `forced_below` をどうセットするか」は `12-distribution.md`。

境界が曖昧になるケース（例: アプリと crate を同タイミングで release する場合の version 整合）は、両ドキュメントの該当箇所が互いに参照しあう形で解決する。本 README にも横断的考慮事項として追記する。

---

## 横断的考慮事項

現時点では **無し**。Rust crate と Electron アプリと npm パッケージはそれぞれ独立に進化させる前提。version 整合・同時 release・tag の統一などのニーズが顕在化した時点で本節に追加する。

候補として将来検討するもの（着手時期未定）:

- Rust crate の major bump と Electron アプリの user-visible 変更の調整（midori-core ABI 変更がアプリ側にどう波及するか）
- `@midori/runtime-{platform}` の npm version と内部に同梱する Rust binary の version 関係
- workspace 全体の release ノート / CHANGELOG をまとめるか各 track 個別に持つかの方針

これらは「実際にコンフリクトを起こした最初のリリース」で議論するのが妥当。現時点で先回りして規約化はしない。
