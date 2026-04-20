# ドライバー・SDK — 技術検討

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## 基本方針

すべてのドライバーを**外部プロセス（プラグイン）**として扱う。built-in という概念は持たない。

MIDI・OSC を含むすべてのドライバーがプラグインとして実装される。リアルタイム性の問題は通信方式を「共有メモリ + ロックフリーリングバッファ」にすることで解決する。

---

## 通信アーキテクチャ：2チャンネルモデル

ドライバーとブリッジの通信を目的別に2つのチャンネルに分離する。

```
Bridge → Driver : stdin（制御コマンド。非リアルタイム）
Driver → Bridge : 共有メモリ リングバッファ（イベント。リアルタイム）
```

```
                       共有メモリ（イベント）
Driver プロセス A ──→ [SPSC リングバッファ A] ──→ Bridge tick スレッド
Driver プロセス B ──→ [SPSC リングバッファ B] ──→ （drain して処理）
Driver プロセス C ──→ [SPSC リングバッファ C] ──→

Bridge → Driver stdin : {"type":"connect","config":{...}}
Driver → Bridge stdout: {"type":"ready"} / {"type":"error",...}
```

制御（接続・切断・設定）は JSON Lines で stdin/stdout を使う。遅延は問題にならない。  
イベント（音符・CC・センサー値）は共有メモリ経由で書き込む。OS スケジューラを介さない。

### 出力側の同期

```
Bridge tick
  1. 計算
  2. 出力リングバッファに書く
  3. セマフォを叩く  ← 出力ドライバーへの合図

出力ドライバー
  セマフォを待つ → 起きる → 読む → デバイスに送信
```

---

## 技術的妥当性

### 共有メモリ + SPSC リングバッファの書き込み遅延

| 手段 | 書き込み遅延 | MIDI 要件（1〜3 ms）との比較 |
|---|---|---|
| stdin/stdout パイプ | 0.5〜5 ms | ❌ ジッタでスパイクあり |
| Unix domain socket | 0.05〜0.5 ms | △ ギリギリ |
| **共有メモリ SPSC** | **0.01〜0.1 µs** | ✅ 要件の 10,000 倍の余裕 |

共有メモリへの書き込みは CPU キャッシュラインの操作（〜5 ns）＋メモリフェンス（〜50 ns）で完了する。OS スケジューラは関与しない。

これは JACK・PipeWire・CoreAudio が内部で採用している確立された手法であり、プロオーディオ用途で実績がある。

### SPSC リングバッファの設計

ドライバーごとに独立した SPSC（Single Producer Single Consumer）バッファを割り当てる。ドライバー間で書き込み競合が発生しない。

```
イベント構造体（約 40 バイト）:
  timestamp : u64   （ナノ秒）
  event_type: u8    （noteOn / noteOff / cc / sysex 等）
  channel   : u8
  param1    : u16   （ノート番号 / コントローラ番号 等）
  param2    : u16   （ベロシティ / 値 等）
  data      : [u8; 28]  （SysEx や拡張データ用）

リングバッファサイズ: 1024 イベント × 40 バイト = 40 KB / ドライバー
10 ドライバー同時接続: 400 KB（実用上問題ない）
```

### 共有メモリのセットアップ手順

```
1. Bridge 起動時に名前付き共有メモリを作成
   Unix:    /dev/shm/midori-{session-id}
   Windows: CreateFileMapping("midori-{session-id}")

2. Bridge がレイアウトマップ（各ドライバースロットのオフセット）を
   共有メモリの先頭に書き込む

3. ドライバープロセスを起動する際、共有メモリ名とスロット番号を引数で渡す
   例: midori-driver-midi --shm /dev/shm/midori-abc123 --slot 0

4. ドライバーが共有メモリをマップし、自スロットのリングバッファに書き込み開始
```

### クロスプラットフォーム対応

| OS | 共有メモリ API | Rust クレート |
|---|---|---|
| macOS / Linux | `mmap` + POSIX shm | `memmap2` |
| Windows | `CreateFileMapping` / `MapViewOfFile` | `memmap2`（Windows 対応済み） |

`memmap2` クレートは Unix・Windows 双方を同一 API で扱える。

### クラッシュ検出

ドライバーが共有メモリの自スロットにハートビートカウンタを書く（100 ms ごと）。Bridge の監視スレッドがカウンタの停滞を検出したらそのドライバーをクラッシュ扱いにし、スロットを解放してランタイムエラーとして記録する。

### 採用根拠まとめ

| 方式 | 採用 | 理由 |
|---|---|---|
| stdin/stdout のみ | ❌ | MIDI 等の 1〜3 ms 要件を満たせない |
| 動的ライブラリ（dylib） | ❌ | クロスプラットフォーム ABI が複雑・クラッシュがブリッジを巻き込む |
| WebAssembly | ❌ | OS の MIDI/BLE API にアクセスできない |
| **共有メモリ SPSC + stdin 制御** | ✅ | リアルタイム要件を満たす・プロセス分離を維持・実装言語不問 |

---

## Driver SDK

共有メモリの操作・リングバッファへの読み書き・セマフォ・ハートビートといったボイラープレートをすべて **Driver SDK**（`midori-driver-sdk` crate）に隠蔽する。

ドライバー開発者はデバイス固有のロジックだけを書けばよい。

```rust
// ドライバー開発者が書くコード（入力ドライバーの例）
fn main() {
    let conn = DriverConnection::connect_from_env(); // 引数を自動解決

    midi_device.on_note_on(|ch, note, vel| {
        conn.push_event(DriverEvent::NoteOn { ch, note, vel }); // これだけ
    });
}
```

SDK が隠す処理：
- 共有メモリのマップ（`memmap2`）
- 自分のリングバッファスロットへの書き込み
- セマフォの待機・通知（出力ドライバー）
- ハートビートカウンタの更新

### 他言語対応

SDK は Rust crate を核とし、C FFI バインディング経由で任意言語から利用できる。MIDI ドライバーは Rust、BLE ドライバーは Python、カスタムハードウェアは C++ など、得意な言語でドライバーを書けるようになる。

```
midori-driver-sdk（Rust crate）
  └── C FFI バインディング
        ├── Python バインディング（PyO3）
        ├── Node.js バインディング（napi-rs）
        └── その他（Go / C++ / 任意の C FFI 対応言語）
```

公式ドライバー（`@midori/driver-midi`・`@midori/driver-osc`）も同じ SDK を使って実装することで、SDK の品質と API 設計を自然に検証する。

---

## バイナリ配布

すべてのドライバーを `@midori/runtime` と同様に npm の optionalDependencies パターンで配布する。

```
@midori/driver-midi          ← MIDI（公式プラグイン）
@midori/driver-osc           ← OSC（公式プラグイン）
some-org/midori-driver-ble   ← コミュニティプラグイン
```

公式プラグイン（midi・osc）はブリッジとは独立してバージョン管理される。

### インストールフロー

```
1. ユーザーが Preferences > プラグイン から URL を入力
2. git clone → midori-plugin.yaml を読んで type: driver を検出
3. GUI が npm install @some-org/midori-driver-xxx を実行
4. プラットフォーム別バイナリが取得される
5. ブリッジ起動時にドライバーバイナリをサブプロセスとして起動し
   共有メモリスロットを割り当てる
```

コードを含むプラグインのインストール時に以下を表示する:
- プラグインが実行するバイナリのパス
- npm パッケージの出所（スコープ・バージョン）
- 「このプラグインはコードを含みます。信頼できる提供者からのみインストールしてください」
