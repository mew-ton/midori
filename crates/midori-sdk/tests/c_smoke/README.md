# C smoke test for midori-sdk

`midori-sdk` が公開する C ABI（`midori_sdk_spsc_*`）が、外部 C コンパイラ /
リンカ経由で実際に成立することを確認する smoke test です。`cargo test
-p midori-sdk` の一部として実行されます。

## 内容

- `spsc_round_trip.c` — `cbindgen` が生成した `midori_sdk.h` を `#include`
  して、`midori_sdk_spsc_init` / `midori_sdk_spsc_push` /
  `midori_sdk_spsc_pop` の round-trip を確認する。失敗時は非ゼロで exit。
- `tests/c_smoke.rs`（リポジトリの 1 つ上）— ホスト C コンパイラを検出して
  `spsc_round_trip.c` をコンパイル、midori-sdk の staticlib をリンク、
  生成バイナリを起動して exit code が 0 であることを assert する。

## ローカル実行時の前提

ホストに C コンパイラ（`gcc` / `clang` / 環境変数 `CC` で指定したもの）が
必要です。Linux / macOS の典型的な開発環境にはデフォルトで利用可能な C
コンパイラが入っていますが、minimal な container などで未インストールの
場合は test が **skip** され、ビルド全体は失敗しません（warning のみ）。

```sh
# Linux: build-essential 同梱の gcc で十分
sudo apt install build-essential

# macOS: xcrun が利用可能であれば clang が使える
xcode-select --install

# 任意のコンパイラを指定する場合
CC=clang cargo test -p midori-sdk
```

## Out of Scope

- Windows MSVC でのリンク検証（Linux / macOS 上で通れば本 smoke test の範囲）
- C++ 互換性（`extern "C++"`）
- CI runner の C コンパイラ環境整備（別 Issue で扱う）
