# midori-sdk

Driver SDK for the [Midori](https://github.com/mew-ton/midori) signal bridge.

ドライバー作者は本クレートを唯一の依存に追加するだけで、`midori-core` の
公開型・SPSC 共有メモリ通信・規約準拠の `<driver> list` / `<driver> start`
CLI を構築できる。

## 含まれるもの

- `midori-core` の全公開型を re-export（`Value` / `Signal` / `IpcEvent` 等）
- 単一プロデューサ単一コンシューマのロックフリー SPSC リングバッファ実装
- `Driver` トレイト + `run()` エントリポイントによる CLI スキャフォールド
- C ABI エクスポート（`midori_sdk_spsc_*` 関数群）と `cbindgen` による C ヘッダ自動生成

## 例: 最小ドライバー

```rust
use midori_sdk::driver::{self, ControlCommand, DeviceEntry, Driver, DriverError};

struct MyDriver;

impl Driver for MyDriver {
    fn list_devices(&mut self) -> Vec<DeviceEntry> {
        vec![DeviceEntry {
            value: "demo-device".into(),
            label: "Demo Device".into(),
        }]
    }

    fn handle_command(&mut self, _cmd: ControlCommand) -> Result<(), DriverError> {
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

fn main() -> std::process::ExitCode {
    driver::run(MyDriver)
}
```

## C ヘッダ

`build.rs` がビルド時に `cbindgen` で `midori_sdk.h` を `OUT_DIR` に生成する。
他言語バインディング（PyO3 / napi-rs / 純粋な C 等）からは下記のように利用する:

```c
#include "midori_sdk.h"

size_t size  = midori_sdk_spsc_storage_size();
size_t align = midori_sdk_spsc_storage_alignment();
void* storage = aligned_alloc(align, size);
midori_sdk_spsc_init(storage);

// `RingSlot` のレイアウトは midori-core::shm::RingSlot を参照（forward 宣言のみ）
// Producer 側
midori_sdk_spsc_push(storage, &slot);
// Consumer 側
midori_sdk_spsc_pop(storage, &out_slot);

// 使い終わったら必ず解放する（aligned_alloc に対応する free）
free(storage);
```

ヘッダの `RingSlot` は `typedef struct RingSlot RingSlot;` の forward 宣言のみで、
構造体の中身は `midori-core` のソース定義を参照すること。これは安定した
バイナリレイアウト（`#[repr(C)]`）を SPSC バッファ内で共有するための制約。

## SPSC 規律

`midori_sdk_spsc_push` / `_pop` は **同時に 1 スレッドからのみ** 呼ぶこと。
違反時の挙動は未定義。Rust API では `SpscStorage::split(&mut self)` が
型レベルで強制する。

## ライセンス

MIT OR Apache-2.0

## 関連

- 設計ドキュメント: [`design/10-driver-plugin.md`](https://github.com/mew-ton/midori/blob/main/design/10-driver-plugin.md)
- リポジトリ構成: [`design/14-repository-structure.md`](https://github.com/mew-ton/midori/blob/main/design/14-repository-structure.md)
