# Layer 4 — デバイス構成（出力）要件

## 責務

Signal を出力ドライバー固有の raw events に変換する。送信先は持たない。

Layer 2（入力）と同一スキーマ（デバイス構成）を使用する。`binding.input` / `binding.output` サブセクションで方向を分離する。

## インターフェース

```
入力: Signal（Layer 3 出力）
出力: raw events（Layer 5 へ渡す出力ドライバー固有のイベント）
```

---

## Layer 2 との対称性

| | Layer 2（入力） | Layer 4（出力） |
|---|---|---|
| `definition` | デバイスの物理構成 | 同じ（出力デバイスの構成） |
| `binding` | raw events → ComponentState | Signal → raw events |
| `layout` | Preview（入力可視化） | Monitor（出力可視化） |

**definition / layout の要件は Layer 2 と共通。**
→ [layers/02-input-recognition/definition-requirements.md](../02-input-recognition/definition-requirements.md)
→ [layers/02-input-recognition/layout-requirements.md](../02-input-recognition/layout-requirements.md)

---

## セクション構成

| セクション | Runtime | GUI | 必須 |
|---|---|---|---|
| `definition` | 使用 | 使用 | ✅ |
| `binding` | 使用 | 静的表示のみ | ✅ |
| `layout` | 不使用 | Monitor 表示に使用 | ❌（なければ自動生成） |

---

## binding セクション要件

入力の binding（[binding-requirements.md](../02-input-recognition/binding-requirements.md)）と鏡対称の構造。

| # | 要件 | 補足 |
|---|---|---|
| 1 | `driver` フィールドによって `to` の型（有効フィールド）が確定すること | `osc` / `midi` など |
| 2 | `definition` の内容によって `from.target` の有効パスが確定すること | definition に存在しないパスはエラー |
| 3 | 1エントリ = 1アクションとすること | 同じ Signal から複数のイベントを出す場合はエントリを分ける |
| 4 | `from` に Signal（ComponentState パス）を、`to` に出力イベント形式を記述する構造とすること | from → to の関係を明示する |
| 5 | `from.target` のパスは component type によって形式が異なること | keyboard は `<id>.<note>.<value>` / それ以外は `<id>.<value>` |
| 6 | keyboard の `{note}` テンプレートを使えること | per-key の出力に展開される |
| 7 | `from.condition` による出力条件の絞り込みをサポートすること | 省略時は値が変化するたびに送出 |
| 8 | binding を変更した場合はブリッジの再起動が必要であること | 起動時にのみ読み込むため |
| 9 | 送信先（ホスト・ポート等）を持たないこと | 送信先はプロファイルが担う |
| 10 | `direction: input` のデバイス構成をプロファイルの出力側に設定した場合はエラーとすること | 起動時バリデーション |
| 11 | `to` フィールドの `value` 指定時は `from.target` の型・range とドライバーの物理値域から逆正規化すること | 入力側の正規化（物理値 → ComponentState）と対称。各ドライバーは逆正規化対象フィールドの物理値域を仕様として定義する |

### `from.target` パスの形式

入力 binding の `to.target` と同じ記法。

| component type | パス形式 | 例 |
|---|---|---|
| `keyboard` | `<component_id>.{note}.<value_name>` | `upper.{note}.pressed` |
| `slider` | `<component_id>.<value_name>` | `upper_expression.value` |
| `switch` | `<component_id>.<value_name>` | `upper_sustain.pressed` |

---

## 設定仕様

→ [config/02-device-config.md](../../config/02-device-config.md)
