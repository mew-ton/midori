# アダプター種別定義 仕様: osc-vrchat

VRChat の OSC アバターパラメーター専用の アダプター種別定義。`osc` ドライバーを基底とし、VRChat 固有の自動正規化・アドレス制約・追加設定フィールドを宣言する。独立したドライバーではない。

アダプター種別定義 の概念と配布方法 → [`../../10-driver-plugin.md`](../../10-driver-plugin.md)

## osc との違い

| 項目 | `osc` | `osc-vrchat` |
|---|---|---|
| 値域 | 不定（明示必須） | VRChat 型ごとに既知（自動正規化可） |
| アダプターの生成 | 手動 | アバターパラメーター JSON から自動生成 |
| OSC アドレス形式 | 任意 | `/avatar/parameters/<ParameterName>` 固定 |
| `set` 省略 | 不可 | definition の `valueType` と `range` が揃っていれば省略可 |

---

## サポート方向

| 方向 | サポート | 備考 |
|---|---|---|
| `input` | ✅ | VRChat → ブリッジ（受信ポート 9001） |
| `output` | ✅ | ブリッジ → VRChat（送信ポート 9000） |

---

## VRChat パラメーター型と値域

VRChat が送受信する OSC パラメーターの型は以下の3種類に限られる。

| VRChat 型 | OSC 型 | 値域 |
|---|---|---|
| `bool` | `b` | `true` / `false` |
| `int` | `i` | `0–255` |
| `float` | `f` | `0.0–1.0` |

---

## binding.input

以下の binding は `driver: osc, adapter_kind: osc-vrchat` のコンテキストで記述する。

```yaml
binding:
  input:
    driver: osc
    adapter_kind: osc-vrchat
    mappings:
      - ...
```

### from フィールド

| フィールド | 必須 | 説明 |
|---|---|---|
| `target` | ✅ | OSC アドレスパターン。フルパス（`/avatar/parameters/UpperExpression`）または短縮形（`UpperExpression`）で記述できる。短縮形はアダプター種別定義の `address_prefix` が自動付与される。`{note}` キャプチャ変数を含められる |
| `type` | ❌ | `bool` / `int` / `float`。省略時は型を問わず処理する |

### set の自動正規化

`set` を省略した場合、`from.type` と `to.target` の definition（`valueType` + `range`）をもとに自動正規化する。

| `from.type` | definition の `valueType` | 自動正規化の挙動 |
|---|---|---|
| `float` | `float` | VRChat 値域 `0.0–1.0` → component の `range` へ線形マッピング |
| `int` | `int` | VRChat 値域 `0–255` → component の `range` へ線形マッピング |
| `bool` | — | `true` / `false` を直接代入（正規化なし） |

**自動正規化の前提**: `to.target` の component が `valueType` と `range` を宣言していること。宣言がない場合は `set` または `setMap` の明示が必要。

```yaml
# set 省略 → float 0.0–1.0 を definition の range [0, 1] へ自動正規化
- from:
    target: /avatar/parameters/UpperExpression
    type: float
  to:
    target: expression.value   # valueType: float, range: [0, 1] が定義されている前提

# set 省略 → int 0–255 を definition の range [0, 15] へ自動正規化
- from:
    target: /avatar/parameters/SceneIndex
    type: int
  to:
    target: scene_index.value   # valueType: int, range: [0, 15] が定義されている前提

# bool は正規化なし・直接代入（set 省略可）
- from:
    target: /avatar/parameters/upper_key_{note}
    type: bool
  to:
    target: upper_key.{note}.pressed
```

---

## binding.output

`osc` ドライバーと同一の構文。`address` にはフルパス（`/avatar/parameters/<ParameterName>`）または `address_prefix` による短縮形（`UpperExpression`）を使える。

`mirror` による逆写像も `osc` と同様に導出できる。

```yaml
binding:
  output:
    driver: osc
    adapter_kind: osc-vrchat
    mappings:
      - from: { target: expression.value }
        to:   { address: UpperExpression, type: float }
        # → Bridge が /avatar/parameters/UpperExpression に展開
```

---

## アバターパラメーター JSON からのアダプター生成

VRChat はアップロード済みアバターの OSC パラメーター一覧を JSON ファイルとして自動生成する。

```
%APPDATA%\..\LocalLow\VRChat\VRChat\OSC\{userId}\Avatars\{avatarId}.json
```

このファイルを読み込むことで、definition・binding を含むアダプター YAML を自動生成できる。

### JSON の構造（VRChat 仕様）

```json
{
  "id": "avtr_xxxx",
  "name": "My Avatar",
  "parameters": [
    { "name": "UpperExpression", "input": { "address": "/avatar/parameters/UpperExpression", "type": "Float" }, "output": { ... } },
    { "name": "upper_key_60",    "input": { "address": "/avatar/parameters/upper_key_60",    "type": "Bool"  }, "output": { ... } }
  ]
}
```

### 生成ルール

| JSON の `type` | 生成される definition の component | `valueType` / `range` |
|---|---|---|
| `Bool` | `switch`（または `keyboard` の `pressed`） | — |
| `Float` | `slider` | `valueType: float, range: [0, 1]` |
| `Int` | `slider` | `valueType: int, range: [0, 255]` |

パラメーター名のパターン（例: `upper_key_{note}` の `{note}` 部分）は AI が推定して `keyboard` コンポーネントに集約する。パターンが判断できない場合は個別コンポーネントとして生成する。

生成後は `metadata.spec` にアバターのパラメーター設計意図を記述することを推奨する。

---

## プロファイルの接続設定

### osc ドライバーの接続フィールド（プロファイル共通）

| フィールド | 必須 | 説明 |
|---|---|---|
| `host` | ✅ | 送信先ホスト（通常 `127.0.0.1`） |
| `port` | ✅ | 送信先ポート（通常 `9000`） |
| `listen_port` | ❌ | 受信ポート（通常 `9001`）。`input` 方向を使う場合に必要 |

これらは `osc` ドライバーの `connection_fields` として宣言されるフィールドであり、osc-vrchat 固有ではない。

### osc-vrchat アダプター種別定義の追加フィールド（additional_fields）

| フィールド | 必須 | 説明 |
|---|---|---|
| `avatar_params` | ❌ | アバターパラメーター JSON のパス。アダプター生成・サジェストに使用 |

このフィールドは osc-vrchat アダプター種別定義の `additional_fields` 宣言によってプロファイルの接続設定に追加される。
