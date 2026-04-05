# Preferences（非配布）

環境固有の設定。マシンごとに異なる。

```yaml
# preferences.yaml

device_bindings:
  # MIDI: OS が返すデバイス名で特定
  - type: midi
    device_name: "ELS-03 Series"    # 部分一致
    input_profile: input/els03.yaml

  # HTTP: 待ち受けポートで特定
  - type: http
    port: 8080
    input_profile: input/my-controller.yaml

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
  input_profile:  input/els03.yaml
  mapper:         mappers/my-avatar.yaml
  output_profile: output/vrchat-default.yaml
  transport:      vrchat-local
```

## device_bindings の type 別フィールド

| type | 必須フィールド | 意味 |
|---|---|---|
| `midi` | `device_name` | OS が返すデバイス名（部分一致） |
| `http` | `port` | 入力サーバーが待ち受けるポート番号 |
| `osc` | `port` | OSC 受信ポート番号 |
