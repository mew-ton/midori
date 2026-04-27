# SDK バインディング API 設計（C / Node.js / Python）

> ステータス：設計フェーズ
> 最終更新：2026-04-27

`midori-sdk` のドライバー作者向け API を、Rust に加えて C / Node.js / Python から自然に書けるよう設計する。本ドキュメントは API の **形** を確定させるものであり、実装は本ドキュメント承認後に言語ごとに別 Issue として切る。

具体ユースケースとして **MIDI / OSC ドライバー** を満たすことを起点にする。MIDI のリアルタイム性（< 1 ms）と OSC のネットワーク I/O 特性が同じ API で吸収できることが妥当性の最低ライン。

---

## 用語: SDK / バインディング / FFI レイヤー

- **SDK**: ドライバー作者が依存するライブラリの総称。Rust では `midori-sdk` クレート、Python では `midori-sdk` PyPI パッケージ、Node.js では `@midori/sdk` npm パッケージ、C では `midori_sdk.h` ＋ 共有ライブラリ
- **バインディング**: 任意の非 Rust 言語から `midori-sdk` Rust 実装を呼び出すためのコード（PyO3 / napi-rs / 手書き C ラッパー）
- **L1 / L2 / L3**: 本ドキュメントで定義する 3 レイヤー（次節）

`midori-driver-sdk` という呼称は `design/10-driver-plugin.md` に残るが、実体クレート名は `midori-sdk`。本ドキュメントは実体名に合わせて `midori-sdk` と書く。

---

## 3 レイヤーモデル（L1 / L2 / L3）

```text
┌─────────────────────────────────────────────────────────┐
│ L3  エンドユーザー API（言語ごとに自然な形）            │
│     - Python: class Driver, async iterator, raise       │
│     - Node:   class Driver, EventEmitter, Promise       │
│     - C:      midori_driver_t + コールバック関数        │
│     - Rust:   trait Driver（既存）                      │
│                                                         │
│     ↑ 各言語のラッパー（L2）が L1 を抽象化              │
├─────────────────────────────────────────────────────────┤
│ L2  言語固有ラッパー                                    │
│     - Python: PyO3 で書く extension module              │
│     - Node:   napi-rs で書く native addon               │
│     - C:      薄いインライン関数 + マクロ               │
│     - Rust:   なし（L1 = L3）                           │
│                                                         │
│     ↑ extern "C" の薄い関数群を呼ぶ                     │
├─────────────────────────────────────────────────────────┤
│ L1  生 FFI（midori-sdk crate の extern "C"）            │
│     - midori_sdk_spsc_*       （MEW-37 で実装済み）     │
│     - midori_sdk_run          （本設計で追加）          │
│     - midori_sdk_emit_*       （本設計で追加）          │
│     - midori_sdk_log          （本設計で追加）          │
│                                                         │
│     ↑ Rust 実装（既存）                                 │
├─────────────────────────────────────────────────────────┤
│ L0  Rust 実装                                           │
│     - midori_sdk::driver::run / run_protocol            │
│     - midori_sdk::spsc::Producer/Consumer               │
│     - midori_core::shm::RingSlot, value_tag             │
└─────────────────────────────────────────────────────────┘
```

| レイヤー | 言語数 | 責務 | 公開 ABI 安定度 |
|---|---|---|---|
| L0 Rust 実装 | 1 | プロトコル本体・SPSC・signal 処理 | 内部 |
| L1 生 FFI | 1（C ABI） | L0 を `extern "C"` で公開 | semver major で破壊変更 |
| L2 言語ラッパー | n | L1 をその言語の慣用形に変換 | L1 が変わったら追従 |
| L3 エンドユーザー API | n | ドライバー作者が触る型 | 言語ごとに semver |

**設計の指針:** L3 はドライバー作者の体験を最優先する（言語ごとに違っていてよい）。L1 は最小に保つ（追加のたびに 4 言語で実装が必要なため）。L2 は機械的な変換に近づける。

### Bridge ↔ Driver プロトコルとの対応

`design/10-driver-plugin.md` の通信アーキテクチャは下記 3 経路。本設計の各レイヤーが扱う経路を明示する。

| 経路 | 方向 | プロトコル | 担当レイヤー |
|---|---|---|---|
| stdin | Bridge → Driver | JSON Lines（hello_ack / connect / disconnect / configure） | **L1 が完全に隠蔽** |
| stdout | Driver → Bridge | JSON Lines（hello / 制御応答）＋ デバッグログ行 | **L1 が完全に隠蔽**（ログ API を除く） |
| 共有メモリ | Driver → Bridge | SPSC リングバッファ | **L1 公開**（`midori_sdk_emit_*` でラップ） |

**hello / hello_ack ハンドシェイクは L1 内に閉じる。** ドライバー作者（L3）はハンドシェイクを意識しない。`midori_sdk_run` の戻り（または "ready" コールバック）で「Bridge と接続完了」とみなせる。Bridge から非互換と返された場合は L1 が ABI 互換のエラーコードで return し、L2 が言語固有の例外/エラーに変換する。

---

## L3 エンドユーザー API（言語別）

各言語の「ドライバー作者から見えるコア型」を提示する。実装例は次節。

### Rust（既存）

```rust
trait Driver {
    fn list_devices(&mut self) -> Vec<DeviceEntry>;
    fn handle_command(&mut self, cmd: ControlCommand) -> Result<(), DriverError>;
    fn shutdown(&mut self) -> Result<(), DriverError>;
}

fn run<D: Driver>(driver: D) -> ExitCode;
```

イベント送出は `Producer::push(slot)` を直接呼ぶ。

### Python

```python
class Driver(Protocol):
    def list_devices(self) -> list[DeviceEntry]: ...
    def on_connect(self, device: str, config: dict) -> None: ...
    def on_disconnect(self) -> None: ...
    def on_configure(self, config: dict) -> None: ...
    def on_shutdown(self) -> None: ...

# 同期 API: イベントは emit_* でメインスレッドから送る
def run(driver: Driver) -> int: ...

# 非同期 API: async for で stdin コマンドを受けつつ自前で I/O ループを回す
async def run_async(driver: Driver) -> None: ...

# イベント送出（どちらの API でも使える。スレッドセーフ）
def emit_int(device_id: str, specifier: str, value: int) -> bool: ...
def emit_float(device_id: str, specifier: str, value: float) -> bool: ...
def emit_bool(device_id: str, specifier: str, value: bool) -> bool: ...
def emit_pulse(device_id: str, specifier: str) -> bool: ...
```

### Node.js

```ts
interface Driver {
    listDevices(): DeviceEntry[];
    onConnect?(device: string, config: unknown): Promise<void> | void;
    onDisconnect?(): Promise<void> | void;
    onConfigure?(config: unknown): Promise<void> | void;
    onShutdown?(): Promise<void> | void;
}

// メインの Promise は Bridge との接続が終わるまで resolve しない
function run(driver: Driver): Promise<void>;

// イベント送出（同期・スレッドセーフ）。戻り値は false=リング満杯
function emitInt(deviceId: string, specifier: string, value: number): boolean;
function emitFloat(deviceId: string, specifier: string, value: number): boolean;
function emitBool(deviceId: string, specifier: string, value: boolean): boolean;
function emitPulse(deviceId: string, specifier: string): boolean;
```

### C

```c
typedef struct midori_device_entry {
    const char* value;
    const char* label;
} midori_device_entry_t;

typedef struct midori_driver_callbacks {
    /* 構造体サイズ。L2 が sizeof(midori_driver_callbacks_t) を必ず代入する。
       L1 はこの値を見て、自分が知らない末尾フィールドへのアクセスを抑止する。
       詳細は「C ABI と struct_size」節を参照 */
    size_t struct_size;

    /* devices に out_count 個のエントリを書き、書いた数を返す */
    size_t (*list_devices)(void* user, midori_device_entry_t* devices, size_t cap);

    /* device, config_json は L1 の所有。コールバック内のみ有効 */
    int (*on_connect)(void* user, const char* device, const char* config_json);
    int (*on_disconnect)(void* user);
    int (*on_configure)(void* user, const char* config_json);
    int (*on_shutdown)(void* user);
} midori_driver_callbacks_t;

/* オプション。NULL なら全てデフォルト値 */
typedef struct midori_run_options {
    size_t struct_size;
    /* 0=L1 が SIGTERM/SIGINT を捕捉する（デフォルト・スタンドアロン用途）
       1=L1 はシグナルを触らない。ホスト側で midori_sdk_trigger_shutdown を呼ぶ */
    int disable_signal_handlers;
} midori_run_options_t;

/* main() からこれを呼ぶだけで <driver> list/start CLI が完成する */
int midori_sdk_run(int argc, char** argv,
                   const midori_driver_callbacks_t* cb, void* user,
                   const midori_run_options_t* opts /* nullable */);

/* シグナルハンドラを使わない埋め込み環境（GUI 内ランタイム等）から
   on_shutdown を起動するための明示 API。midori_sdk_run の中で
   コマンドループに「shutdown 要求」を立てる。べき等 */
void midori_sdk_trigger_shutdown(void);

/* イベント送出。スレッドセーフ。0=リング満杯 / 1=成功 */
int midori_sdk_emit_int(const char* device_id, const char* specifier, int64_t v);
int midori_sdk_emit_float(const char* device_id, const char* specifier, double v);
int midori_sdk_emit_bool(const char* device_id, const char* specifier, int v);
int midori_sdk_emit_pulse(const char* device_id, const char* specifier);
```

---

## ドライバー実装コード例（MIDI / OSC × 3 言語）

「理想的な実装が 10〜30 行で書ける」ことが API の質を判定する基準。下記は擬似コード（インポート・エラー処理は省略あり）。

### 例 1: Python × MIDI（mido を併用）

```python
import mido
from midori_sdk import Driver, run, emit_int, emit_pulse

class MidiDriver:
    def list_devices(self):
        return [{"value": n, "label": n} for n in mido.get_input_names()]

    def on_connect(self, device: str, config: dict):
        self.device = device
        self.port = mido.open_input(device, callback=self._on_msg)

    def _on_msg(self, msg: mido.Message):
        # MIDI コールバックは mido のスレッドで動く。emit_* はスレッドセーフ。
        d = self.device
        if msg.type == "note_on" and msg.velocity > 0:
            emit_int(d, f"upper.{msg.note}.velocity", msg.velocity)
        elif msg.type == "note_off" or (msg.type == "note_on" and msg.velocity == 0):
            emit_pulse(d, f"upper.{msg.note}.released")
        elif msg.type == "control_change":
            emit_int(d, f"cc.{msg.control}.value", msg.value)
        elif msg.type == "pitchwheel":
            emit_int(d, "pitch.value", msg.pitch)

    def on_disconnect(self): self.port.close()
    def on_shutdown(self):   self.port.close()

if __name__ == "__main__":
    raise SystemExit(run(MidiDriver()))
```

### 例 2: Node.js × OSC（osc.js を併用）

```ts
import * as osc from "osc";
import { run, emitFloat, emitInt, emitBool } from "@midori/sdk";

class OscDriver {
    listDevices() { return [{ value: "udp:0.0.0.0:9000", label: "OSC UDP listener" }]; }

    async onConnect(device: string, config: { port: number }) {
        this.udp = new osc.UDPPort({ localAddress: "0.0.0.0", localPort: config.port });
        this.udp.on("message", (m) => this.dispatch(m));
        this.udp.open();
    }

    private dispatch(m: { address: string; args: { type: string; value: unknown }[] }) {
        const arg = m.args[0];
        const spec = `osc.${m.address.replace(/^\//, "").replace(/\//g, ".")}`;
        if (arg.type === "f")        emitFloat("osc", spec, arg.value as number);
        else if (arg.type === "i")   emitInt("osc",   spec, arg.value as number);
        else if (arg.type === "T" || arg.type === "F")
                                     emitBool("osc",  spec, arg.type === "T");
    }

    async onDisconnect() { this.udp?.close(); }
    async onShutdown()   { this.udp?.close(); }
}

run(new OscDriver()).catch((e) => { console.error(e); process.exit(1); });
```

### 例 3: C × MIDI（PortMidi 併用、最小骨格）

```c
#include <midori_sdk.h>
#include <portmidi.h>

static PortMidiStream* stream;
static const char* device_id;

static size_t list_devices(void* u, midori_device_entry_t* out, size_t cap) {
    int n = Pm_CountDevices(); size_t k = 0;
    for (int i = 0; i < n && k < cap; i++) {
        const PmDeviceInfo* info = Pm_GetDeviceInfo(i);
        if (info->input) { out[k].value = info->name; out[k].label = info->name; k++; }
    }
    return k;
}

static int on_connect(void* u, const char* dev, const char* cfg_json) {
    device_id = dev;
    /* dev → PmDeviceID 解決は省略 */
    return Pm_OpenInput(&stream, /*resolved id*/ 0, NULL, 256, NULL, NULL);
}

/* MIDI ポーリングスレッドから呼ばれる前提（簡略化） */
static void poll_loop(void) {
    PmEvent ev[64];
    while (Pm_Read(stream, ev, 64) > 0) {
        int status = Pm_MessageStatus(ev[0].message) & 0xF0;
        int data1  = Pm_MessageData1(ev[0].message);
        int data2  = Pm_MessageData2(ev[0].message);
        if (status == 0x90 && data2 > 0)        midori_sdk_emit_int(device_id, "noteOn",  data1);
        else if (status == 0x80 || status == 0x90) midori_sdk_emit_pulse(device_id, "noteOff");
        else if (status == 0xB0)                midori_sdk_emit_int(device_id, "cc",      data2);
    }
}

int main(int argc, char** argv) {
    midori_driver_callbacks_t cb = {
        .struct_size = sizeof(midori_driver_callbacks_t),
        .list_devices = list_devices,
        .on_connect = on_connect,
        /* on_disconnect / on_configure / on_shutdown は NULL のまま（未対応） */
    };
    return midori_sdk_run(argc, argv, &cb, NULL, /* opts */ NULL);
}
```

> 補足: C 例ではポーリングスレッドの起動を省略。実装時は `on_connect` で `pthread_create` などを使う。L1 はスレッドモデルを規定しない（次節「スレッド/非同期モデル」）。

### MIDI / OSC が網羅される論点

| 論点 | カバー方法 |
|---|---|
| MIDI note-on/off | Python 例で `emit_int(velocity)` / `emit_pulse(released)` を使い分け |
| MIDI CC | Python 例 / C 例で `emit_int("cc.<n>.value")` |
| MIDI pitch bend | Python 例で `emit_int("pitch.value")`（int14 を i64 に格納） |
| OSC アドレスパターン | Node 例で `/foo/bar` → `osc.foo.bar` の SignalSpecifier 化 |
| OSC 型付き引数 | Node 例で `f` → emitFloat、`i` → emitInt、`T/F` → emitBool |

---

## ライフサイクルとハンドシェイク

```text
[GUI/Bridge]                    [Driver L3]                  [SDK L1]
     │                               │                            │
     │  spawn <driver> start         │                            │
     │ ───────────────────────────►  │                            │
     │                               │  run(callbacks)            │
     │                               │ ─────────────────────────► │
     │                               │                            │  write hello
     │  ◄───────────────────────────────────────────────────────  │  to stdout
     │                               │                            │
     │  hello_ack(compatible:true)   │                            │
     │ ───────────────────────────────────────────────────────►   │
     │                               │   on_connect(dev, config)  │
     │                               │ ◄────────────────────────  │  ※compatible=true 後
     │                               │                            │
     │  ── shared-memory events ──────────────────────────────►  Bridge
     │                               │   emit_int / emit_pulse 等  │
     │                               │                            │
     │  configure / disconnect       │                            │
     │ ───────────────────────────────────────────────────────►   │
     │                               │   on_configure / on_disconnect
     │                               │ ◄────────────────────────  │
     │                               │                            │
     │  SIGTERM                      │                            │
     │ ───────────────────────────────────────────────────────►   │
     │                               │   on_shutdown              │
     │                               │ ◄────────────────────────  │
     │                               │                            │
     │                               │  process exits             │
```

**所属レイヤーまとめ:**

| 段階 | 所属レイヤー | 備考 |
|---|---|---|
| `hello` 送信 | L1 | `run()` 呼び出し時に自動 |
| `hello_ack` 受信 | L1 | 非互換なら L1 が即 return（L2 が例外化） |
| `connect` / `disconnect` / `configure` のディスパッチ | L1 → L2 → L3 | L3 は受け取るだけ |
| イベント送出（共有メモリ） | L3 → L1 | L2 は型変換のみ |
| ログ出力（stdout 非 JSON 行） | L3 → L1 | `midori_sdk_log(level, message)` |
| シグナルハンドラ | L1（opt-out 可） | デフォルト: SIGTERM / SIGINT で `on_shutdown` を呼んでから return。`disable_signal_handlers=1` を指定するとホストから `midori_sdk_trigger_shutdown()` で起動 |

---

## イベントループモデル（言語別の決定）

| 言語 | 採用モデル | 理由 |
|---|---|---|
| Rust | コールバック（既存 `Driver` トレイト） | trait の `&mut self` で SPSC 規律を担保しつつ、ドライバー作者が自由に worker thread を持てる |
| C | コールバック（関数ポインタ） | 言語に組込みのイベントループがない最小公倍数。スレッドモデルは作者に委ねる |
| Python | コールバック（同期 `run`）＋ オプションで `async` イテレータ（`run_async`） | 多くの MIDI/OSC ライブラリ（mido / python-osc）はコールバック前提。`async` を必須にすると依存ライブラリと衝突する。`run_async` は Trio/asyncio ユーザー向けの便利層 |
| Node.js | コールバック（Promise 化された L3）＋ 内部は async | Node の I/O は async が自然。`run()` は Promise を返し、各 `on*` も `Promise<void>` を返せる。`emitFloat` などは同期 |

**共通方針:** L1 は **コールバック**を唯一のディスパッチ方式とする。L2 が必要に応じて async / iterator に変換する。

`emit_*` はすべての言語で **同期・スレッドセーフ** とする。SPSC は単一プロデューサーのため、L1 内部に Mutex を 1 つ持って protect する。複数スレッドからの同時 emit は順序保証なし（L1 は到着順で push する）。

**ただし Node.js のみ例外**: `emitFloat` などは **JS メインスレッドからの呼び出しのみサポート**する（`worker_threads` の Worker から直接呼ぶことは未サポート）。napi-rs の制約と、Worker から呼ぶ場合に必要な `ThreadsafeFunction` ベースの追加 API が L1 の単一 emit ハンドル前提と整合しないため。Worker からイベントを送りたい場合は、メインスレッド側に `MessageChannel` 等で転送してから `emitFloat` を呼ぶ運用とする。Python（threading / native thread）と C（pthread）は任意のスレッドから呼んで安全。

> Note: SPSC の「単一プロデューサー」規律は **共有メモリへの書き込み権を持つのが Driver プロセス 1 つだけ** という意味。Driver プロセス内で複数スレッドから emit したい場合は L1 内 Mutex で直列化する。これは性能より安全側に倒した妥協で、リアルタイム要件を満たさないなら将来 thread-local バッファ + ドレインに置き換える（L1 ABI 変更なしで実装差し替え可能）。

---

## エラーモデル（言語別の決定）

| 言語 | 採用モデル | 例 |
|---|---|---|
| Rust | `Result<(), DriverError>`（既存） | `Err(DriverError::new("port not found"))` |
| C | 関数戻り値の `int`（0=ok, 非 0=エラー）＋ `midori_sdk_last_error()` でメッセージ取得 | `if (Pm_OpenInput(...) != pmNoError) return -1;` |
| Python | 例外（`MidoriError` をベースに細分化） | `raise MidoriDeviceNotFound(name)` |
| Node.js | `throw` / `Promise.reject`（Error サブクラス） | `throw new MidoriDeviceNotFound(name)` |

### エラー伝播経路

```text
L3 のコールバック内で発生したエラー
    ↓ L2 が言語固有のエラー → 共通の C 文字列に変換
    ↓ L1 が IpcEvent::Log{level: Error, layer: "driver", message} として stdout に書き出す
    ↓ L1 が Bridge に "fatal" 判定を伝えるためコールバック return value を負値で返す
Bridge がプロセス終了を検知して再起動 / 停止
```

**ハンドシェイク前のエラー（L1 内部エラー含む）:**

| 状況 | C 戻り値 | Python | Node |
|---|---|---|---|
| `hello_ack(compatible:false)` | `-2`（IncompatibleSDK） | `MidoriIncompatibleSdkError` | `MidoriIncompatibleSdkError` |
| stdin EOF before `hello_ack` | `-3`（HandshakeMissing） | `MidoriHandshakeError` | `MidoriHandshakeError` |
| stdin パース失敗 | `-4`（ProtocolParseError） | `MidoriProtocolError` | `MidoriProtocolError` |
| L3 コールバックがエラーを返した | `-5`（DriverError） | `MidoriDriverError`（元例外を `__cause__` で保持） | `MidoriDriverError`（`cause` を保持） |

**`emit_*` の戻り値:** 0=リング満杯（ドロップされた）、1=成功。リング満杯は **エラーではなく back-pressure シグナル** として扱う。L3 はドロップ件数をログに出すだけでよい（再送ロジックは持たない方が単純）。

---

## スレッド / 非同期モデル

### SPSC 規律と複数スレッド emit の整合

| プロセス | スレッド | アクセス可能な操作 |
|---|---|---|
| Driver | 任意の数のスレッド | `emit_*` をどこから呼んでも安全（L1 内 Mutex で直列化） |
| Driver | 制御スレッド（`run()` を呼んだスレッド） | `on_connect` / `on_disconnect` / `on_configure` / `on_shutdown` のディスパッチを受ける |
| Bridge | 1 スレッド（消費者） | リングから pop |

L1 内の Mutex は **共有メモリ書き込みの直前**で取る短命なもの。MIDI のリアルタイム性（< 1 ms）への影響は小さいが、ハードリアルタイム要件には届かない。リアルタイムが厳しいユースケースが出てきたら、`midori_sdk_emit_batch` のような複数スロットまとめ書き API か、thread-local バッファに切替（L1 ABI 拡張）。

### Python の GIL

PyO3 の慣例どおり、`emit_*` は GIL を **解放してから** L1 を呼ぶ（`Python::allow_threads`）。コールバック（`on_connect` 等）は Python 側にいる時間が長いため、L2 がコールバック呼び出し時のみ GIL を取る。これにより、別スレッドから `emit_*` を呼ぶ MIDI コールバックモデルが GIL に詰まらない。

### Node.js のイベントループ

napi-rs の `ThreadsafeFunction` を使い、L1 から非メインスレッドで来たイベントは Node のイベントループにポストする。具体的には:

- `on_connect` / `on_disconnect` / `on_configure` / `on_shutdown` は L1 → ThreadsafeFunction → JS メインスレッド
- `emitFloat` などは JS メインスレッドから直接 Rust を同期呼び出し（イベントループへの post 不要）

### スレッド方針まとめ

| 言語 | コールバックが走るスレッド | emit_* が呼べるスレッド |
|---|---|---|
| Rust | `run()` を呼んだスレッド | 任意（`Producer` を `Send` で送れば） |
| C | `midori_sdk_run` を呼んだスレッド | 任意 |
| Python | `run()` を呼んだ Python スレッド（GIL あり） | 任意（GIL 解放して呼ぶ） |
| Node | JS メインスレッド | JS メインスレッド（worker からは napi-rs の追加 API が必要、本設計では未対応） |

---

## L1 FFI 拡張：暫定シグネチャ一覧

MEW-37 で実装済みのものに加え、本設計を実現するために追加が必要な extern "C" 関数。

### 既存（MEW-37 で実装済み）

```c
size_t midori_sdk_spsc_storage_size(void);
size_t midori_sdk_spsc_storage_alignment(void);
void   midori_sdk_spsc_init(void* storage);
uint8_t midori_sdk_spsc_push(const void* storage, const RingSlot* slot);
uint8_t midori_sdk_spsc_pop(const void* storage, RingSlot* out_slot);
```

### 追加（本設計）

```c
/* バージョン情報 */
const char* midori_sdk_version(void);

/* CLI ランナー（list / start を内部でディスパッチ）
   コールバック構造体・ランオプションともに先頭に struct_size を持つ。
   詳細は「C ABI と struct_size」節 */
typedef struct midori_driver_callbacks {
    size_t struct_size;  /* L2 が sizeof(midori_driver_callbacks_t) を代入 */
    size_t (*list_devices)(void* user, midori_device_entry_t* devices, size_t cap);
    int (*on_connect)(void* user, const char* device, const char* config_json);
    int (*on_disconnect)(void* user);
    int (*on_configure)(void* user, const char* config_json);
    int (*on_shutdown)(void* user);
} midori_driver_callbacks_t;

typedef struct midori_run_options {
    size_t struct_size;
    int disable_signal_handlers;  /* 1 で SIGTERM/SIGINT を触らない */
} midori_run_options_t;

int midori_sdk_run(int argc, char** argv,
                   const midori_driver_callbacks_t* cb, void* user,
                   const midori_run_options_t* opts /* nullable */);

/* 埋め込みホストから on_shutdown を起動する明示 API（べき等） */
void midori_sdk_trigger_shutdown(void);

/* イベント送出（内部で SPSC ハンドルを保持） */
int midori_sdk_emit_int  (const char* device_id, const char* specifier, int64_t v);
int midori_sdk_emit_float(const char* device_id, const char* specifier, double v);
int midori_sdk_emit_bool (const char* device_id, const char* specifier, int v);
int midori_sdk_emit_pulse(const char* device_id, const char* specifier);
int midori_sdk_emit_null (const char* device_id, const char* specifier);

/* デバッグログ（stdout の非 JSON 行として出力） */
typedef enum { MIDORI_LOG_INFO = 0, MIDORI_LOG_WARN = 1, MIDORI_LOG_ERROR = 2 } midori_log_level_t;
int midori_sdk_log(midori_log_level_t level, const char* message);

/* スレッドローカルなエラーメッセージ（C 専用ヘルパー） */
const char* midori_sdk_last_error(void);
```

### 設計上の注意

- **`emit_*` は SPSC ハンドルを引数に取らない。** L1 内部に Bridge と共有するハンドル（プロセス起動時に Bridge から fd で渡される）を 1 個持ち、`midori_sdk_run` の中で初期化する。複数 emit hand を持つ用途は当面なし
- **文字列はすべて UTF-8 NUL 終端。** L1 → L2 でコピーが発生する経路があるので、ホットパスでは長い specifier を避ける運用 docs を別途用意する（`device_id <= 63B`、`specifier <= 127B` の `RingSlot` 制約も明記）
- **`config_json` は L1 が文字列のまま L2/L3 に渡す。** JSON のパースは言語側で行う（Python なら `json.loads`、Node なら `JSON.parse`）。L1 が型を持たないことで、ドライバー固有の `config` スキーマ拡張が L1 ABI 変更を要求しない
- **`list_devices` のメモリ:** ドライバーが返す `value` / `label` ポインタは **コールバック return まで有効** であればよい。L1 がコールバック内で JSON にシリアライズし stdout に書き出す
- **シグナル処理は opt-out 可能。** `midori_sdk_run` はデフォルトで SIGTERM / SIGINT を捕捉して `on_shutdown` を呼んでから return する（スタンドアロンドライバー用途）。ただし `midori_run_options_t::disable_signal_handlers = 1` を渡すと L1 はシグナルに触らず、ホスト（GUI 内ランタイム埋め込み等）が `midori_sdk_trigger_shutdown()` を呼んでループを終了させる。**L1 は OS シグナルを「黙って奪わない」** ことで、ホストランタイムの既存シグナル制御と衝突しないようにする

### C ABI と `struct_size`

C 側に公開する **コールバック構造体（`midori_driver_callbacks_t`）と オプション構造体（`midori_run_options_t`）はすべて先頭に `size_t struct_size` を持つ。** L2 は `struct_size = sizeof(midori_driver_callbacks_t)` を構築時に必ず代入する。L1 はこの値を見て、自分が知る最も新しいフィールドが `struct_size` の範囲内に含まれるか確認する。

```c
// L1 側の擬似コード
if (cb->struct_size < offsetof(midori_driver_callbacks_t, on_configure)
                      + sizeof(cb->on_configure)) {
    // 古いヘッダで作られた構造体。on_configure は読まない（古い L2 の場合は NULL ガード相当）
}
```

これにより:

- **構造体末尾への新フィールド追加は semver minor で行える**（旧バイナリは `struct_size` で守られる）
- 旧 L2 が `struct_size` を 0 のまま送る危険を避けるため、L1 は `struct_size == 0` を **不正値としてエラー return** する
- L2 ラッパー（PyO3 / napi-rs）は単純に `mem::size_of` を入れるだけなのでミスしにくい

`#[non_exhaustive]` を Rust 側 `ControlCommand` に付ける効果（バリアント追加が非破壊）と、`struct_size` を構造体に付ける効果（フィールド追加が非破壊）は別物だが、両方を組み合わせることで「Rust 側 + L1 ABI ともに新コマンド追加が semver minor」となる。

---

## Driver トレイトとの整合性

`design/10-driver-plugin.md` の Rust `Driver` トレイト（既存実装）と本設計のレイヤーモデルを揃える。

| Rust `Driver` のメソッド | L3（他言語）での対応 | 差分 |
|---|---|---|
| `list_devices(&mut self) -> Vec<DeviceEntry>` | `list_devices()` / `listDevices()` | 同一 |
| `handle_command(&mut self, ControlCommand)` | `on_connect` / `on_disconnect` / `on_configure` の 3 メソッドに分解 | **差分**: 他言語ではバリアント分解する方が自然なため。Rust 側を分解するかは別 Issue（破壊変更になるため慎重） |
| `shutdown(&mut self) -> Result<(), DriverError>` | `on_shutdown()` | 同一 |
| なし | `emit_*` 関数群（モジュールレベル） | 他言語ではトレイトの代わりにモジュール関数で提供 |

**`handle_command` のバリアント分解は L2 で行う。** L1 は Rust と同様の単一エントリ（`on_command`）でも良かったが、`ControlCommand` の variant 数が増えるたびに 4 言語の L2 を改修するのを避けたかった。L1 は variant ごとに別の C 関数ポインタを持つ。

将来 `ControlCommand` に新 variant を足すときの手順:

1. Rust 側で `ControlCommand` に variant を追加（`#[non_exhaustive]` のため非破壊）
2. L1 に対応する関数ポインタを `midori_driver_callbacks_t` の **末尾** に追加（`struct_size` ガードで旧バイナリは新フィールドを読まれないため、**semver minor**）
3. L2 がそれを各言語の慣用形に変換し、新フィールドへのポインタを `NULL` または実装関数で埋める
4. 旧ヘッダでビルドされたドライバーは新 variant の関数ポインタが構造体に存在しないが、L1 は `struct_size` で範囲外を判定して呼び出さない

---

## 既存 Rust ドライバー実装との整合

`crates/midori-driver-midi` / `crates/midori-driver-osc` は現状 `fn main(){}` のスタブ。本設計の妥当性検証は次のいずれかで行う:

1. **Rust 実装を先行**: 公式 MIDI/OSC ドライバーを Rust で完成させ、`Driver` トレイト経由で動くことを確認 → その後 Python/Node/C ラッパーで同等のコードを書く
2. **3 言語並走**: Python/Node/C それぞれで MIDI/OSC の最小実装を本設計の API で書き、サンプルとしてリポジトリに置く

実務では (1) を推奨（Rust 側の完成度が L1 の要件を駆動するため）。ただし、3 言語の API 形が「Rust より自然 / 同等」であることを確かめる目的で (2) のサンプルも各言語の後続 Issue で必須とする（後述「後続 Issue 案」）。

---

## Value / Signal 型の十分性チェック

本設計が要求する MIDI / OSC のメッセージが `midori-core` の `Value` で表現できるかを検証する。

### `Value` の現在の variant

```rust
enum Value { Bool(bool), Pulse, Int(i64), Float(f64), Null }
```

### MIDI

| MIDI 概念 | 表現方法 | 過不足 |
|---|---|---|
| Note On velocity | `Value::Int(0..=127)`（`upper.{note}.velocity`） | OK |
| Note Off | `Value::Pulse`（`upper.{note}.released`） | OK |
| Control Change value | `Value::Int(0..=127)`（`cc.{n}.value`） | OK |
| Pitch Bend value | `Value::Int(-8192..=8191)` | OK |
| Channel Aftertouch | `Value::Int(0..=127)` | OK |
| Program Change | `Value::Int(0..=127)` | OK |
| Real-Time（start/stop/clock） | `Value::Pulse` | OK |
| **SysEx 可変長ペイロード** | ✗（`Value` に bytes バリアントがない） | **不足** |

**結論: `Value::Bytes(Vec<u8>)` が必要かは別 Issue。** 現行 `binding/midi.md` の SysEx 仕様は `pattern` でバイト列をマッチさせ `setMap` で `Int` に正規化する方式のため、ドライバーから上に流れる時点では Int に落ちている。**ドライバーが `Value::Bytes` を発行する経路は当面不要**。RingSlot 上に bytes フィールドが無い物理制約からも、当面は不要と判断する。

### OSC

| OSC 概念 | 表現方法 | 過不足 |
|---|---|---|
| `f`（float32） | `Value::Float(f64)` | OK（精度上昇のみ） |
| `i`（int32） | `Value::Int(i64)` | OK |
| `T` / `F`（bool） | `Value::Bool` | OK |
| `s`（string） | ✗ | **不足**（後述） |
| `b`（blob） | ✗ | **不足** |
| `t`（timetag） | ✗ | **不足** |
| **アドレスパターン** | `SignalSpecifier` の `component_id.path` にエンコード | OK（命名規約に依存） |
| **バンドル（時刻同期）** | 個別メッセージに分解して emit | OK（時刻同期は失う） |

**結論: `Value::String` / `Value::Bytes` / `Value::Time` は本設計のスコープ外で扱う。** 理由:

1. 現行 `binding/osc.md` は `float` / `int` / `bool` のみ。string/blob/timetag 受け取りの YAML 構文がない
2. 拡張するなら **midori-core の `Value` 拡張 + `RingSlot` レイアウト変更（major bump）** が必要で、本ドキュメント単独で決めるべきでない
3. 本ドキュメントの **Notes** として明記し、別 Issue で扱う（後述「Notes: core への拡張提案」）

### Notes: core への拡張提案

本設計の API 形を保ったまま将来 OSC string / blob を扱うには、`midori-core::shm::RingSlot` に **可変長補助領域**（slot 外の共有メモリプール）を導入する必要がある。これは `midori-core` の major bump イベントなので、別の設計ドキュメント `design/16-value-extensions.md`（仮）で扱うのが妥当。本設計はそれが起きるまで `Bool/Pulse/Int/Float/Null` の範囲で MIDI/OSC（数値型のみ）をカバーする。

---

## 後続 Issue 案（Out of Scope の延長）

本設計の承認後、以下の単位で実装 Issue を切ることを推奨する。「言語別」ではなく **L1 拡張を先に固める** のが鍵。

### Phase 1: L1 FFI 拡張（前提）

| Issue 案 | 内容 | 想定 SP |
|---|---|---|
| L1-1 | `midori_sdk_run` / `midori_driver_callbacks_t` の Rust 実装と extern "C" エクスポート | 5 |
| L1-2 | `midori_sdk_emit_int/float/bool/pulse/null` 実装＋ Bridge との fd 受け渡しプロトコル | 3 |
| L1-3 | `midori_sdk_log` 実装と stdout 非 JSON 行への書き出し | 2 |
| L1-4 | cbindgen の更新と C ヘッダ自動生成テスト拡張 | 2 |

### Phase 2: 言語ラッパー（並列可）

| Issue 案 | 内容 | 想定 SP |
|---|---|---|
| C-1 | C サンプルプロジェクト（PortMidi 連携の MIDI ドライバー） | 5 |
| C-2 | C サンプルプロジェクト（liblo 連携の OSC ドライバー） | 3 |
| Py-1 | PyO3 で `midori-sdk` Python パッケージ実装 | 5 |
| Py-2 | Python × MIDI（mido）/ OSC（python-osc）サンプル | 3 |
| Node-1 | napi-rs で `@midori/sdk` 実装 | 5 |
| Node-2 | Node × MIDI（midi）/ OSC（osc.js）サンプル | 3 |

### Phase 3: 配布・公式ドライバー（並列可）

| Issue 案 | 内容 | 想定 SP |
|---|---|---|
| Dist-1 | PyPI 配布フロー（maturin） | 3 |
| Dist-2 | npm 配布フロー（@midori/sdk-{platform}） | 3 |
| Dist-3 | C 共有ライブラリの GitHub Releases バイナリ | 2 |
| Drv-1 | `midori-driver-midi` の Rust 実装本体 | 8 |
| Drv-2 | `midori-driver-osc` の Rust 実装本体 | 8 |

「言語別」と「機能別」の両軸で切れる位置を意図的に Phase で区切る:

- **L1 が固まる前に L2/L3 に着手すると、L1 ABI を 4 言語ぶん何度も改修することになる**（高コスト）
- C/Py/Node は Phase 2 内で並列可（L1 が共通基盤）
- 公式 MIDI/OSC ドライバー（Drv-1/2）は L2 ラッパーの妥当性検証のために Phase 2 と並走させても良い

### スコープに **入れない** もの（再掲）

- 配布インフラの最終確定（`12-distribution.md` 系）
- バージョニング戦略の semver 境界線確定
- DMX / Art-Net / HID / Serial 等の追加ドライバー領域

---

## 参考リンク

- `design/10-driver-plugin.md` — Driver トレイト・通信アーキテクチャ・SDK 位置づけ
- `design/14-repository-structure.md` — `midori-sdk` クレートの責務
- `design/12-distribution.md` — 配布方針（参考のみ）
- `crates/midori-sdk/src/driver.rs` — Rust `Driver` トレイトと CLI スキャフォールド実装
- `crates/midori-sdk/src/ffi.rs` — MEW-37 で導入された L1 FFI（SPSC のみ）
- `crates/midori-core/src/shm.rs` — `RingSlot` レイアウト・`value_tag`
- `design/config/drivers/midi.md` / `osc.md` — MIDI/OSC binding 構文
