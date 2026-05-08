# tick レートが波形系入力を吸収できるか

> ステータス：観察メモ
> 最終更新：2026-05-08

## 観察

Midori tick は実時間ベースで 1ms（1000Hz）目安に動く（[`../layers/cross/timing.md`](../layers/cross/timing.md)）。一方、音声系のフレーム長は典型的に 5–20ms（FFT 256 sample @ 48kHz ≒ 5.3ms、`audio-voice` の `frame_ms` で 10–20ms）。

つまり **driver 内で特徴抽出を完結させて frame 単位の値を吐く構成**（[`./audio-drivers.md`](./audio-drivers.md) の方針）であれば、tick 側で乗るのは ≤ 1ms に収まり、合計レイテンシは音声フレーム長で律速される。tick 側がボトルネックにならないという観察。

```text
driver 側: マイク → 5–20ms 分のサンプルバッファ → FFT / ML 推論 → 特徴量 1 個（FFT bands / viseme weights / volume）
                                                                       │
                                                                       ▼ SPSC ring（< tick 1 個分）
tick 側:  ≤ 1ms で drain → mapper graph 評価 → 出力
```

## 制約

サブ tick 精度を要求すると成立しない。

- tick 周期 1ms に対して 48kHz サンプル周期 20.8µs。1 tick 内に約 48 サンプルが積まれる
- timing 仕様上、同一 tick 内に積まれた同一フィールドへの書き込みは **後勝ち**（[`../layers/cross/timing.md`](../layers/cross/timing.md)「tick 内に複数イベントが積まれた場合」）
- したがって生サンプルを `float` フィールドに 48kHz で書き込んでも、1 tick あたり 1 サンプルしか mapper 側からは観測できない
- 波形そのものを mapper graph に乗せたい用途は、driver 側で特徴量化するか、波形を array コンポーネントとして 1 tick 1 batch で扱う新セマンティクスが要る

## この観察が意味すること

- 既存の tick モデルは音声系特徴量入力と素直に整合する。`audio-spectrum` / `audio-voice` のために tick 側を変更する必要はない
- 波形をそのまま mapper に通すユースケースが将来出てくるなら、それは「特徴抽出を mapper でやりたい」というモチベーションの方向であり、tick の高速化ではなく **タイムスタンプ付き buffer / 波形 component** の追加で解く問題（`../layers/cross/timing.md` の「sub-tick タイミングの保持（将来）」と接続する）
