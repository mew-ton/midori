# プロファイル（プライベート）

実行単位。入力デバイス構成・変換グラフ・出力デバイス構成・実デバイス接続設定を束ねる。ブリッジはプロファイルを元に動作する。

```yaml
# profiles/my-setup.yaml

name: エレクトーン → VRChat

inputs:
  - id: yamaha-els03                    # 省略時はデバイスファイルのベース名から自動生成
    device: devices/yamaha-els03.yaml   # 入力デバイス構成ファイル
    connection:
      driver: midi
      device_name: "ELS-03 Series"      # 実機と部分一致でバインドされる

transform: mappers/my-avatar.yaml       # 変換グラフファイル

outputs:
  - id: vrchat-default                  # 省略時はデバイスファイルのベース名から自動生成
    device: devices/vrchat-default.yaml # 出力デバイス構成ファイル
    connection:
      driver: osc
      host: 127.0.0.1
      port: 9000
      avatar_params: "C:/Users/.../OSC/.../Avatars/avtr_xxx.json"  # 任意（osc-vrchat config type の追加フィールド）
```

## セクション

| セクション | 必須 | 内容 |
|---|---|---|
| `name` | ❌ | 表示名 |
| `inputs` | ✅ | 入力デバイス構成と接続設定のリスト（1件以上） |
| `inputs[].id` | ❌ | 変換グラフから参照する識別子。省略時はデバイスファイルのベース名（拡張子除く）を自動使用 |
| `inputs[].device` | ✅ | 入力デバイス構成ファイルのパス。ワークスペース内は `devices/foo.yaml`、プラグイン由来は `@<plugin-name>/devices/foo.yaml`（`<app-data-dir>/plugins/<plugin-name>/` に解決） |
| `inputs[].connection` | ✅ | 実デバイスとの接続設定。`driver` で内容が変わる |
| `transform` | ✅ | 使用する変換グラフファイルのパス |
| `outputs` | ✅ | 出力デバイス構成と接続設定のリスト（1件以上） |
| `outputs[].id` | ❌ | 変換グラフから参照する識別子。省略時はデバイスファイルのベース名（拡張子除く）を自動使用 |
| `outputs[].device` | ✅ | 出力デバイス構成ファイルのパス。ワークスペース内は `devices/foo.yaml`、プラグイン由来は `@<plugin-name>/devices/foo.yaml`（`<app-data-dir>/plugins/<plugin-name>/` に解決） |
| `outputs[].connection` | ✅ | 実デバイスとの接続設定。`driver` で内容が変わる |

## connection の driver 別フィールド

| driver | フィールド | 内容 |
|---|---|---|
| `midi` | `device_name` | OS が返すデバイス名（部分一致）。入出力共通 |
| `osc` | `host`, `port`, `listen_port` | 出力時: `host`・`port`（送信先）。入力時: `listen_port`（待ち受けポート）。双方向の場合は全て指定 |

Device Config Type（例: `osc-vrchat`）が `additional_fields` を宣言している場合、それらのフィールドも接続設定に追加される。`avatar_params` 等は osc-vrchat config type が宣言した追加フィールドであり、このテーブルには含まない。詳細 → [`../10-driver-plugin.md`](../10-driver-plugin.md)

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
