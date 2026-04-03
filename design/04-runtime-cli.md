# ブリッジ CLI インターフェース

## コマンドライン

```
midori [OPTIONS]

OPTIONS:
  --preferences   <path>   Preferences YAML（デフォルト: ./preferences.yaml）
  --input-source  <path>   Input Source Profile YAML（preferences を上書き）
  --mapper        <path>   Mapper YAML（preferences を上書き）
  --output-target <path>   Output Target Profile YAML（preferences を上書き）
  --log-level     <level>  error | warn | info | debug
  --log-format    <fmt>    text | json
```

## ログフォーマット

ログは JSON 形式で GUI がパースしやすい構造にする。

```json
{"level":"info","layer":"input","driver":"midi","event":"noteOn","channel":1,"note":60,"velocity":100}
{"level":"info","layer":"input-recognition","component":"upper","value_name":"pressed","note":60,"value":true}
{"level":"info","layer":"mapper","signal":"upper_key_60","value":1.0}
{"level":"info","layer":"output-recognition","address":"/avatar/parameters/upper_key_60","type":"float","value":1.0}
{"level":"info","layer":"output","driver":"udp","host":"127.0.0.1","port":9000}
```
