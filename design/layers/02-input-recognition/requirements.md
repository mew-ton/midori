# Layer 2 — アダプター（入力）要件

## 責務

raw events を ComponentState に変換・正規化する。楽器・デバイスの物理構成と値域を定義する。

Layer 4（出力）と同一スキーマ（アダプター）を使用する。`binding.input` / `binding.output` サブセクションで方向を分離する。

## インターフェース

```
入力: raw events（Layer 1 出力）
出力: ComponentState
```

## 要件

| # | 要件 | 補足 |
|---|---|---|
| 1 | definition でデバイスの物理構成（component・value・range）を定義できること | component type: keyboard / slider / knob / toggle 等 |
| 2 | binding で raw events を component.value にマッピングできること | ドット記法で `<component_id>.<value_name>` を指定 |
| 3 | 値をレンジ定義に従い正規化すること | definition の `range` に従って正規化した値を渡す |
| 4 | set による定数代入と setMap による条件分岐代入をサポートすること | setMap の when 記法: 完全一致 / 比較演算子 / 範囲 |
| 5 | binding に存在しない component / value を参照した場合はエラーとすること | 起動時にバリデーションする |
| 5b | `direction: output` のアダプターをプロファイルの入力側に設定した場合はエラーとすること | 起動時バリデーション |
| 6 | layout の変更はブリッジ再起動なしに反映できること | layout は View のみが使用。Runtime は不使用 |
| 7 | binding の変更はブリッジ再起動を必要とすること | Runtime が起動時に binding を読み込むため |
| 8 | 楽器・デバイス1種 = YAML 1ファイルとして公開配布できること | 機種共通の情報のみを持つ。環境固有値は含まない |

## セクション構成

| セクション | Runtime | GUI | 配布 |
|---|---|---|---|
| `definition` | 使用 | 使用 | 必須 |
| `binding` | 使用 | 静的表示のみ | 必須 |
| `layout` | 不使用 | 使用 | 任意（なければ自動生成） |

## セクション別要件

| セクション | ファイル |
|---|---|
| definition | [definition-requirements.md](./definition-requirements.md) |
| binding | [binding-requirements.md](./binding-requirements.md) |
| layout | [layout-requirements.md](./layout-requirements.md) |
| Signal 指定子 | [signal-specifier.md](./signal-specifier.md) |

## 設定仕様

→ [config/02-device-config.md](../../config/02-device-config.md)
