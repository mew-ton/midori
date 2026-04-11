# Layer 2 — definition 要件

入力ソースの物理構成と取りうる値を定義するセクション。Runtime・GUI 双方が使用する。

## 使用者

| 使用者 | 用途 |
|---|---|
| Runtime | binding の参照先として component / value の存在確認・正規化に使用 |
| GUI | エディター表示・binding / layout の補完候補として使用 |

## 要件

| # | 要件 | 補足 |
|---|---|---|
| 1 | 各 component に一意の id を持つこと | binding / layout から参照するキーになる |
| 2 | component の type を指定できること | type によって primitive value が決まる |
| 3 | primitive value は type が確定した時点で暗黙的に存在すること | 宣言不要。binding から常に参照できる |
| 4 | デバイスが対応する場合に限り、追加の value を `additionals:` で宣言できること | 宣言しなければその value は存在しない扱いになる |
| 5 | `float` の value には `range` を必須とすること | `0~1` または `-1~1` |
| 6 | component id と value name の組み合わせはファイル内で一意であること | |
| 7 | ドライバーの種類に依存しない共通構造であること | |

component type の一覧・primitive value・additionals の仕様 → [config/00-component-types.md](../../config/00-component-types.md)

設定仕様（YAML 記法・フィールド定義・サンプル）→ [config/02-device-config.md#definition-セクション](../../config/02-device-config.md#definition-セクション)

---

## key_range 音名記法

```
フォーマット: <音名><オクターブ>
音名: c / c# / db / d / d# / eb / e / f / f# / gb / g / g# / ab / a / a# / bb / b
オクターブ: -1 〜 9

例: c4（Middle C）, a4（A440）, f#3, bb2
```

note と key（音名）が矛盾する場合は note 番号を正とする。

`octave_offset` の仕様（システム基準・補正テーブル）は [デバイス構成](../../config/02-device-config.md#octave_offset) を参照。
