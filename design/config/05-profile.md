# プロファイル（プライベート）

実行単位。入力デバイス構成・変換グラフ・出力デバイス構成・実デバイス接続設定を束ねる。ブリッジはプロファイルを元に動作する。

```yaml
# profiles/my-setup.yaml

name: エレクトーン → VRChat

inputs:
  - id: yamaha-els03                    # 省略時はデバイスファイルのベース名から自動生成
    device: devices/yamaha-els03.yaml   # 入力デバイス構成ファイル
    connection:
      type: midi
      device_name: "ELS-03 Series"      # 実機と部分一致でバインドされる

transform: mappers/my-avatar.yaml       # 変換グラフファイル

outputs:
  - id: vrchat-default                  # 省略時はデバイスファイルのベース名から自動生成
    device: devices/vrchat-default.yaml # 出力デバイス構成ファイル
    connection:
      type: osc-vrchat
      host: 127.0.0.1
      port: 9000
      avatar_params: "C:/Users/.../OSC/.../Avatars/avtr_xxx.json"  # 任意
```

## セクション

| セクション | 必須 | 内容 |
|---|---|---|
| `name` | ❌ | 表示名 |
| `inputs` | ✅ | 入力デバイス構成と接続設定のリスト（1件以上） |
| `inputs[].id` | ❌ | 変換グラフから参照する識別子。省略時はデバイスファイルのベース名（拡張子除く）を自動使用 |
| `inputs[].device` | ✅ | 入力デバイス構成ファイルのパス。ユーザーファイルは `devices/foo.yaml`、プラグイン由来は `@<plugin-name>/devices/foo.yaml` |
| `inputs[].connection` | ✅ | 実デバイスとの接続設定。`type` で内容が変わる |
| `transform` | ✅ | 使用する変換グラフファイルのパス |
| `outputs` | ✅ | 出力デバイス構成と接続設定のリスト（1件以上） |
| `outputs[].id` | ❌ | 変換グラフから参照する識別子。省略時はデバイスファイルのベース名（拡張子除く）を自動使用 |
| `outputs[].device` | ✅ | 出力デバイス構成ファイルのパス。ユーザーファイルは `devices/foo.yaml`、プラグイン由来は `@<plugin-name>/devices/foo.yaml` |
| `outputs[].connection` | ✅ | 実デバイスとの接続設定。`type` で内容が変わる |

## connection の type 別フィールド

| type | フィールド | 内容 |
|---|---|---|
| `midi` | `device_name` | OS が返すデバイス名（部分一致） |
| `osc` | `host`, `port` | 送受信先のホスト・ポート |
| `osc-vrchat` | `host`, `port`, `listen_port`, `avatar_params` | `listen_port` は受信ポート（通常 `9001`）。VRChat → ブリッジ方向を使う場合に必要（任意）。`avatar_params` は VRChat が自動生成するアバターパラメーター JSON のパス（任意） |
| `http` | `port` | 待ち受けポート番号 |

## 接続のバリデーション

プロファイル読み込み時、`device_name` 等で指定された該当ポート・デバイスが OS 上に存在するかをデバイスごとに確認する。動的な ID 解決などの複雑な抽象化は持たず、プロファイルが実環境の接続情報を直接宣言するシンプルな方式をとる。

接続確認の結果はデバイスごとに独立して扱う。

| 状況 | 挙動 |
|---|---|
| 全デバイスが接続済み | ブリッジ起動可能 |
| 一部デバイスが未接続 | 未接続デバイスを灰色表示。ブリッジ起動不可（全デバイス接続が前提） |
| 全デバイスが未接続 | 同上 |

デバイスが接続されると自動検出し、全デバイスが揃った時点で実行ボタンが有効になる。

---

## 実行の制約

同時に実行できるプロファイルは **1つのみ**。
