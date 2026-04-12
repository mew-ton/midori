# プロファイル（プライベート）

実行単位。入力デバイス構成・変換グラフ・出力デバイス構成・実デバイス接続設定を束ねる。ブリッジはプロファイルを元に動作する。

```yaml
# profiles/my-setup.yaml

name: エレクトーン → VRChat

input:
  device: devices/yamaha-els03.yaml   # 入力デバイス構成ファイル
  connection:
    type: midi
    device_name: "ELS-03 Series"      # 実機と部分一致でバインドされる

transform: mappers/my-avatar.yaml     # 変換グラフファイル

output:
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
| `input.device` | ✅ | 使用する入力デバイス構成ファイルのパス |
| `input.connection` | ✅ | 実デバイスとの接続設定。`type` で内容が変わる |
| `transform` | ✅ | 使用する変換グラフファイルのパス |
| `output.device` | ✅ | 使用する出力デバイス構成ファイルのパス |
| `output.connection` | ✅ | 実デバイスとの接続設定。`type` で内容が変わる |

## connection の type 別フィールド

| type | フィールド | 内容 |
|---|---|---|
| `midi` | `device_name` | OS が返すデバイス名（部分一致） |
| `osc` | `host`, `port` | 送受信先のホスト・ポート |
| `osc-vrchat` | `host`, `port`, `avatar_params` | `avatar_params` は VRChat が自動生成するアバターパラメーター JSON のパス（任意） |
| `http` | `port` | 待ち受けポート番号 |

## 接続のバリデーション

プロファイル読み込み時、`device_name` 等で指定された該当ポート・デバイスが OS 上に存在しない場合はロードエラーとなる。動的な ID 解決などの複雑な抽象化は持たず、プロファイルが実環境の接続情報を直接宣言するシンプルな方式をとる。
