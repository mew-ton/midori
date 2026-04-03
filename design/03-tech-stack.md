# 技術スタック

## 構成

| コンポーネント | 技術 | 理由 |
|---|---|---|
| Runtime（ブリッジ） | Rust | GC なし・並列処理・クロスプラットフォームバイナリ |
| GUI | Electron | Node.js から Runtime を直接呼び出せる。クロスプラットフォーム |
| Runtime の配布形式 | npm パッケージ（プラットフォーム別バイナリ） | Electron アプリが依存関係として最適バイナリを取得できる |

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

---

## GUI（Electron）

### 要件

| 要件 | 理由 |
|---|---|
| ブリッジと疎結合であること | GUI がブリッジの入出力に直接触れない構造にする |
| ブリッジのモニタリングができること | 実行中のパイプライン状態・ログをリアルタイムで確認できる |
| ブリッジの設定ファイルを編集できること | Device Profile / Mapper / Preferences の編集 |
| ブリッジを起動・停止できること | GUI からブリッジプロセスを制御できる |

### Runtime との連携

```
Electron（メインプロセス）
└── child_process.spawn("midori", [...args])
      │ stdout（JSON stream）
      ▼
    IPC
      ▼
Electron（レンダラープロセス）
└── Pipeline Monitor / Preview / Monitor タブ
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
