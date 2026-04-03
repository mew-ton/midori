# Preferences（非配布）

環境固有の設定。マシンごとに異なる。

```yaml
# preferences.yaml

device_bindings:
  - device_name: "ELS-03 Series"         # OS が返すデバイス名（部分一致）
    input_source: input-sources/els03.yaml

transports:
  - id: vrchat-local
    driver: udp
    host: 127.0.0.1
    port: 9000

  - id: vrchat-remote
    driver: udp
    host: 192.168.1.10
    port: 9000

pipeline:
  input_source:  input-sources/els03.yaml
  mapper:        mappers/my-avatar.yaml
  output_target: output-targets/vrchat-default.yaml
  transport:     vrchat-local
```
