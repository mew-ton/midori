# セキュリティ設計：ドライバーサンドボックス

> ステータス：調査完了・方針確定中
> 最終更新：2026-04-21
> 対象：Layer 1 / Layer 5 のドライバープロセス（`10-driver-plugin.md` 参照）

## 背景

ドライバーはすべて外部プロセスのプラグインとして実装される（`10-driver-plugin.md`）。GitHub Releases でバイナリ配布され、コミュニティが開発できる。**ドライバーは任意のバイナリであり、ユーザー権限でフルにコードが走る**。

現行の対策はインストール時の警告（L0）のみ。本ドキュメントは Bridge 側が能動的にドライバーを閉じ込める方針を定める。

---

## 脅威モデル

### 想定する攻撃者

| 主体 | 動機 | 例 |
|---|---|---|
| 悪意ある配布者 | 情報窃取・ランサムウェア・クリプトマイナー | `driver-fake-midi` を公開、実体は `~/.ssh` を外部送信 |
| 事故的な実装者 | バグによるクラッシュ・無限ループ・ディスク書き込み | ログを `/` 直下に吐き続ける、再接続ループで CPU 100% |
| サプライチェーン | 信頼できた作者のアカウント乗っ取り | 善良だった `@midori/driver-midi` が更新で悪質化 |
| ネットワーク経由 | ダウンロード経路の改ざん | GitHub Releases のバイナリを中間者で差し替え |

### 信頼境界

```
Bridge（信頼されたコード）
 │
 │  stdin / stdout (JSON Lines) + 共有メモリ  ← 信頼境界
 │
ドライバー（コミュニティ由来・信頼しないコード）
 │
 ▼
OS API（ファイル・ネットワーク・デバイス）
```

Bridge はドライバーを「敵」として扱う。信頼境界をまたぐ入力は：
- ドライバーが stdout に書く JSON Lines
- 共有メモリに書かれるイベント構造体
- ドライバープロセスの終了コード・シグナル

### スコープ外

- Bridge 本体・GUI の脆弱性（別問題）
- ワークスペース YAML の悪性（Bridge が解釈する純データ。ドライバーと独立したリスク）
- ユーザー端末の OS が既に侵害されているケース

---

## アーキテクチャ上の制約

サンドボックス手法を選ぶ前提として、Midori 固有の制約がある。

| 制約 | サンドボックスへの影響 |
|---|---|
| レイテンシ要件 < 10ms（MIDI 区間は 1〜3ms） | syscall / IPC オーバーヘッドが効く |
| 共有メモリ IPC | プロセス隔離度を上げると共有メモリ作成権限が問題になる |
| 物理デバイスアクセス（CoreMIDI / WinMM / ALSA / BLE 等） | 強いサンドボックスほどデバイス通過用の穴が必要 |
| Bridge による自動再起動 | サンドボックス設定は再 spawn 時も再現性を持つ必要がある |
| バイナリ配布（GitHub Releases） | 実行前検証（署名・ハッシュ）とセットで意味を持つ |

ポイントは「**デバイスを触れるが、デバイス以外は触れない**」という方向に絞り込む設計になる点。

---

## 採用方針：段階的実装

### Phase 1：ベースライン強化（L1 達成）

全プラットフォーム共通で常時適用。実装コストが低く効果が確実なもの。

| 対策 | 内容 |
|---|---|
| fd 継承の最小化 | `CLOEXEC` / `STARTUPINFOEX` で Bridge 側の機密 fd をドライバーに渡さない |
| 環境変数フィルタ | `PATH` / `HOME` / `TMPDIR` のみ継承。`AWS_*` 等の機密変数を除去 |
| リソースリミット | CPU 時間・メモリ・fd 数の上限（`setrlimit` / Job Object） |
| ワーキングディレクトリの固定 | 相対パス経由のリークを防止 |
| Linux: `PR_SET_NO_NEW_PRIVS` | `prctl` で setuid バイナリ経由の権限昇格を封じる |
| Windows: Job Object | `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` ＋ Mitigation Policy（DEP / ASLR / ACG / CIG） |
| SHA-256 pin | インストール時に GitHub Releases の digest を記録。次回更新時に比較 |

### Phase 2：permission 宣言（L1 → L2 の基盤）

`driver.yaml` に `permissions` セクションを追加する。ブラウザ拡張の manifest permissions と同様のモデル。

```yaml
# driver.yaml（Phase 2 追加フィールド）
name: midi
permissions:
  device:
    - midi
  filesystem:
    - read: "@self/data/**"   # ドライバー自身のバンドルディレクトリのみ
  network: []                 # 未宣言 = 全拒否
  process:
    - spawn: false
    - max_memory_mb: 128
```

インストール時の警告を「権限リスト表示」に具体化できる：

```
この midi-plugin は以下の権限を要求しています：
  ・MIDI デバイスへのアクセス
  ・ネットワーク：送信なし／受信なし
  ・ファイルシステム：プラグイン自身のディレクトリの読み取り
  ・プロセス生成：なし
  ・最大メモリ：128 MB

[インストール] [キャンセル]
```

Phase 2 では宣言と表示のみ。OS サンドボックスへの変換（enforcement）は Phase 3。

### Phase 3：OS サンドボックスへの変換（L2〜L3）

Phase 2 の permission 宣言を各 OS のサンドボックス機構に変換して強制する。

| OS | 採用手法 | 概要 |
|---|---|---|
| macOS | `sandbox-exec`（SBPL） | Bridge が SBPL プロファイルを動的生成し `sandbox-exec -p` 経由で起動 |
| Linux | Landlock ＋ seccomp-bpf | Bridge の `pre_exec` で適用（`CommandExt::pre_exec`） |
| Windows | LPAC ＋ Job Object | `rappct` ベースで起動時に SECURITY_CAPABILITIES を構築 |

permission 未宣言の既存ドライバーは「untrusted モード」として最小 capability で起動する。

### Phase 4：純変換系ドライバーの WASM 化（将来）

物理 I/O を持たないドライバー（プロトコル変換・アナライザ等）向け。Extism または wasmtime をホスト組み込みする。Layer 1 / 5 のインターフェース互換を保ちつつ、`release_assets` の代替として `wasm:` キーを追加する想定。

---

## プラットフォーム別詳細

### macOS

**sandbox-exec / Seatbelt（SBPL）**を短期採用する。`man` ページで deprecated と明記されているが、Firefox / Chrome のコンテンツプロセス等が現行も使用しており、Apple が OS 内部で依存しているため即廃止の可能性は低い。

```scheme
(version 1)
(deny default)
(allow process-fork process-exec)
(allow file-read* (subpath "/System/Library"))
(allow file-read* (subpath "<plugin-dir>"))
(allow mach-lookup (global-name "com.apple.midiserver"))
(allow network-outbound (remote ip4 "127.0.0.1:*"))
(allow ipc-posix-shm*)   ; 共有メモリ IPC
```

App Sandbox（公式置換経路）は `.app` バンドル＋署名が前提で CLI バイナリと相性が悪い。配布モデルの見直しとセットで中期採用を検討する。

### Linux

推奨スタック：

```
Bridge が fork →（CommandExt::pre_exec で）
  prctl(PR_SET_NO_NEW_PRIVS)
  setrlimit (RLIMIT_NOFILE / RLIMIT_AS)
  Landlock（ファイル: プラグインディレクトリのみ read）
  Landlock（TCP: 接続先制限）※ UDP は ABI 5 時点で未対応
  seccomp-bpf（syscall allowlist）
  → execve(driver)
```

seccomp allowlist（ドライバー向け最小）の主要項目：
- `read / write / mmap / poll / epoll_*`（基本 I/O）
- `ioctl`（ALSA 制御）
- `socket / bind / sendto / recvfrom / connect`（OSC 用）
- `shm_open / mmap / shmget / shmat`（共有メモリ）
- `execve`・`ptrace`・`clone`（thread 限定を除く）は **拒否**

### Windows

推奨スタック：**LPAC ＋ Job Object ＋ Process Mitigation Policy**。

capability 付与は「プラグインマニフェストで宣言 → Bridge が起動時に SECURITY_CAPABILITIES に反映」。

注意点：
- loopback はデフォルトでブロック。OSC ドライバーには loopback exemption が必要
- Windows MIDI Services（2026）との RPC が LPAC から届くか要確認
- 共有メモリ（Section オブジェクト）は Bridge が DACL で Package SID に ACCESS 許可を付与して作成する

---

## permission → OS サンドボックス変換表

| permission | macOS (SBPL) | Linux | Windows |
|---|---|---|---|
| `device.midi` | `mach-lookup com.apple.midiserver` | `/dev/snd/*` r/w ＋ seccomp ioctl | `humanInterfaceDevice` capability |
| `device.bluetooth` | `mach-lookup com.apple.bluetoothd` | BlueZ D-Bus 穴開け | `bluetooth` capability |
| `network.udp.listen: 9000` | `network-inbound (local udp "*:9000")` | seccomp `bind` | `privateNetworkClientServer` |
| `network.tcp.connect: *` | `network-outbound (remote tcp)` | Landlock TCP connect (ABI 4+) | `internetClient` |
| `filesystem.read: @self/...` | `file-read* (subpath ...)` | Landlock `PathBeneath(RO)` | ACL エントリ |
| `process.spawn: false` | `(deny process-fork)` | seccomp `clone/fork/execve` 拒否 | `JOB_OBJECT_LIMIT_ACTIVE_PROCESS=1` |

---

## レイテンシへの影響

| 手法 | MIDI 1〜3ms への影響 |
|---|---|
| sandbox-exec / Landlock / seccomp-bpf / LPAC | 実質ゼロ（チェックは syscall 境界で 1 回、ホットパスではキャッシュ） |
| user namespace（bwrap） | 初回起動時のみ数 ms。常駐後は無影響 |
| WASM | ホットパスで 1.1〜1.3×、GC / コンパイル待ちが最悪ケース |

---

## 未解決事項

| 項目 | 内容 |
|---|---|
| macOS App Sandbox への移行 | deprecated の sandbox-exec を継続するか、バンドル配布を強制して App Sandbox に寄せるか |
| Windows MIDI Services 2026 | LPAC からのサービス経由 RPC が届くか。`com` capability 付与の必要性 |
| Windows LPAC と共有メモリ | Section の DACL 設計。LPAC だと通常の AppContainer より制約が厳しい |
| BLE / USB のクロス OS capability | 3 OS で語彙が全く違うため最小公倍数の設計が難しい |
| permission 語彙の正規化 | `device.midi` / `device.bluetooth` 等を SDK 側で OS 依存の書き方に落とす抽象層 |
| 署名配布 | GitHub Artifact Attestations（2024 GA）を前提にできるか |
| 再起動時のサンドボックス再適用 | Bridge 自動再起動時にプロファイルを安定的に再構築できるか |
| SDK のインターフェース | Landlock / seccomp をドライバー実装者に書かせるか、SDK が自動適用するか |
