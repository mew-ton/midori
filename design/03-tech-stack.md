# 技術スタック

## 構成

| コンポーネント | 技術 | 理由 |
|---|---|---|
| Runtime（ブリッジ） | Rust | GC なし・並列処理・クロスプラットフォームバイナリ |
| Runtime の配布形式 | npm パッケージ（プラットフォーム別バイナリ） | Electron アプリが依存関係として最適バイナリを取得できる |
| GUI シェル | Electron | Node.js から Runtime を直接呼び出せる。クロスプラットフォーム |
| GUI サーバー | Astro SSR（Node） | 設定ファイルの読み書き・SSR レンダリング |
| 設定エディター | Svelte（Astro Island） | データ構造由来の動的 UI |
| 監視コンポーネント / 接続線 | pure JS + CSS / SVG | 動的描画。フレームワーク不介在 |

---

## Runtime（Rust）

### 要件

| 要件 | 理由 |
|---|---|
| 並列処理に強いこと | 入力・変換・出力の各層を独立して並行実行できる必要がある |
| GC による処理停止が発生しないこと | 演奏のリアルタイム同期において GC pause による spike は許容できない |
| クロスプラットフォーム（Windows / macOS 最低限） | ユーザーの環境を限定しない |
| CLI バイナリとして単体実行できること | GUI なしでも動作し、GUI からはプロセスとして起動・終了できること |

### npm パッケージとしての配布

esbuild・Biome 等と同様のパターンで、プラットフォーム別のオプショナルパッケージを用意する。

```
@midori/runtime                    ← メタパッケージ（各プラットフォームパッケージを optionalDependencies で列挙）
@midori/runtime-win32-x64         ← Windows x64 バイナリ
@midori/runtime-darwin-x64        ← macOS Intel バイナリ
@midori/runtime-darwin-arm64      ← macOS Apple Silicon バイナリ
@midori/runtime-linux-x64         ← Linux x64 バイナリ
```

Electron アプリは `@midori/runtime` を依存関係に追加するだけで、インストール時に npm が現在のプラットフォームに対応するバイナリを自動取得する。

公式ドライバープラグイン（`@midori/driver-midi`・`@midori/driver-osc`）は Git リポジトリ ＋ GitHub Releases で配布する（npm は使用しない）。

**Electron 起動時のプロビジョニング**: Electron は起動のたびに公式ドライバーのバージョンを確認し、`<app-data-dir>/plugins/` にインストールされていない（またはバージョンが古い）場合は自動でインストールする。これにより `midori` CLI を単体起動した場合も `<app-data-dir>/plugins/` を参照するだけで公式ドライバーが見つかる。コミュニティプラグインはユーザーが GUI から別途インストールする。

---

## GUI（Electron + Astro SSR）

### 要件

| 要件 | 理由 |
|---|---|
| ブリッジと疎結合であること | GUI がブリッジの入出力に直接触れない構造にする |
| ブリッジのモニタリングができること | 実行中のパイプライン状態・ログをリアルタイムで確認できる |
| ブリッジの設定ファイルを編集できること | デバイス構成 / 変換グラフ / Preferences の編集 |
| ブリッジを起動・停止できること | GUI からブリッジプロセスを制御できる |

### Electron + Astro SSR 構成

Electron のメインプロセスでローカル Node サーバー（Astro SSR）を起動し、レンダラーは `http://localhost:PORT` を表示する。

```
Electron メインプロセス
├── Astro SSR サーバー（Node）を起動
├── midori バイナリを child_process.spawn で起動
└── BrowserWindow → http://localhost:PORT

Astro SSR サーバー
├── 設定エディターページ   YAML を読んでサーバーサイドレンダリング
└── 監視コンポーネント      Astro Island として配置
```

### フロントエンドの設計原則

動的描画の性質によって技術を使い分ける。

| 種別 | 技術 | 原則 |
|---|---|---|
| **動的描画**（描画・アニメーション・リアルタイム更新） | pure JS + CSS | フレームワーク・VDOM・差分検出を一切介在させない |
| **データ構造由来の描画**（データ構造をもとに UI を構築・編集する） | Svelte（Astro Island） | フレームワークのリアクティビティに任せる |

「動的描画」とは、値・位置の変化を視覚に反映する処理全般を指す。

### 各領域への適用

| 領域 | 種別 | 技術 |
|---|---|---|
| Preview / Monitor タブ（監視） | 動的描画 | pure JS（`dataset` 書き換え）+ CSS |
| 変換グラフ 接続線（ベジエ曲線） | 動的描画 | pure JS + 動的 SVG（`<node-wire>` として自律描画） |
| 設定エディター（Definition / Binding 等） | データ構造由来 | Svelte（Astro Island） |
| 変換グラフ ノード本体（パラメーター編集） | データ構造由来 | Svelte（Astro Island） |

#### 動的描画の例：監視コンポーネント

```js
// Runtime からイベント受信 → dataset を書き換えるだけ
element.dataset.pressed = event.value ? "1" : "0"
element.dataset.velocity = event.velocity
```

```css
piano-key[data-pressed="1"] { background: var(--color-active); }
```

#### 動的描画の例：変換グラフ 接続線

`<node-wire>` は `from` / `to` 属性にポート要素を受け取り、`ResizeObserver` で位置変化を監視して SVG ベジエを自動再描画する。Svelte 側はノードを動かすだけでよく、線を関知しない。

```html
<node-wire from="vel_pack.out" to="vel_flatten.in" type="float"></node-wire>
```

### Runtime との連携

```
Electron メインプロセス
├── child_process.spawn("midori", [...args])  → stdout（JSON Lines）を IPC で転送
└── Astro SSR サーバー起動
      ▼
Electron レンダラー（http://localhost:PORT）
├── 監視コンポーネント   SSE（/events）→ dataset 書き換え
└── イベントログ    全イベントをログ表示
```

`@midori/runtime` が提供するバイナリのパスをメインプロセスで解決して `spawn` する。

---

## レイテンシ目標

| ステップ | 目標 |
|---|---|
| raw event → ComponentState | < 0.5ms |
| ComponentState → Signal | < 0.5ms |
| Signal → raw output event → 送信 | < 0.5ms |
| **合計** | **< 10ms** |

VRChat の描画フレーム（11〜14ms / frame）に収まることを要件とする。
