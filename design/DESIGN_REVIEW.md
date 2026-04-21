# Design Review

矛盾チェックで自動修正できなかった項目。設計判断が必要。

---

## 2026-04-21 Round 1–2

### [design/config/02-device-config.md:268] set スカラーリテラルの bool/float target への暗黙変換ルール

`set: <スカラーリテラル>` は「正規化なし・値をそのまま代入」と説明されているが、`int` リテラルを `bool` または `float` target に代入する際の型変換挙動が明示されていない。`set.expr` では「0→false、それ以外→true」と定義されているが、`set:` スカラーの文脈では記述がない。

Option A: `set: スカラー` でも `set.expr` と同じ変換ルール（`bool` target には 0→false, それ以外→true）を適用すると仕様書に明記する。
Option B: `set: スカラー` の `bool` target への int 代入はバリデーションエラーとし、`true`/`false` のみを許容する（`set: 0` / `set: 1` は不正とする）。

### [design/layers/02-input-recognition/binding-requirements.md:58] pitchBend の `{note}` 解決例 `upper.60.lateral`

binding-requirements.md の `{note}` 解決表に `pitchBend` の例として `upper.60.lateral` が記載されているが、`yamaha-els03.yaml` の実装では横タッチは `upper_horizontal` という独立した slider コンポーネントとしてモデル化されており、keyboard の additional value としての `lateral` は存在しない。

Option A: 例を実際の設計（`upper_horizontal.value`）に沿った内容に変更する（`pitchBend` → slider コンポーネントへのマッピングを示す）。
Option B: `lateral` を keyboard の additional value として `yamaha-els03.yaml` の definition に追加し、例通りの実装に合わせる（キー単位の横傾きが将来必要になる可能性も考慮）。

### [design/07-ui-ux/05-device-config.md:158-159] setMap エディターモックアップの `set: 0` / `set: 1`

setMap エディターのUIモックアップで `set: 0` / `set: 1` という int リテラルが使われているが、一般的な bool target（`switch.pressed`）には `false` / `true` が正しい値。`int` target のケースを示したい場合は target の型を明示する必要がある。

Option A: モックアップの例を `set: false` / `set: true` に変更し、bool target の場合を示す。
Option B: モックアップの target を `int` 型に変更（例: `registration.value`）し、`set: 0` / `set: 16` のような例を示す。
