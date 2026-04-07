# Preferences（非配布）

環境固有の設定。マシンごとに異なる。パイプラインの構成（どのデバイス構成・変換グラフを使うか）はプロファイルファイルが担うため、ここには含まない。

```yaml
# preferences.yaml

device_bindings:
  # MIDI: OS が返すデバイス名で特定
  - type: midi
    device_name: "ELS-03 Series"    # 部分一致

  # HTTP: 待ち受けポートで特定
  - type: http
    port: 8080
```

## device_bindings

OS 上の実デバイスと driver タイプを紐づける。プロファイルの「入力デバイス設定」で選択された driver タイプに対して、ここの設定が適用される。

| type | 必須フィールド | 意味 |
|---|---|---|
| `midi` | `device_name` | OS が返すデバイス名（部分一致） |
| `http` | `port` | 入力サーバーが待ち受けるポート番号 |
| `osc` | `port` | OSC 受信ポート番号 |
