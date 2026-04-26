//! Driver CLI スキャフォールド。
//!
//! ドライバー作者は [`Driver`] トレイトを実装し、`main()` から [`run`] を
//! 呼び出すだけで `<driver> list` / `<driver> start` の規約準拠 CLI が
//! 完成する。プロトコルの詳細は `design/10-driver-plugin.md` を参照。
//!
//! # 通信アーキテクチャ
//!
//! - **stdin** (Bridge → Driver): JSON Lines の制御コマンド
//! - **stdout** (Driver → Bridge): `hello` メッセージ + 非 JSON はデバッグログ
//! - **共有メモリ** (Driver → Bridge): リアルタイムイベント（[`crate::spsc`]）
//!
//! 本モジュールは制御チャンネル（stdin/stdout）のみを扱う。リアルタイム
//! イベントの送出は呼び出し側（[`Driver::handle_command`] の `Connect`
//! コマンド処理など）が `Producer` を起動する責務を負う。

use std::io::{BufRead, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// `<driver> list` の出力エントリ。
///
/// `value` はドライバー固有の識別子（`start` 時の `Connect` コマンドで参照される）、
/// `label` はユーザー表示名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceEntry {
    pub value: String,
    pub label: String,
}

/// Driver → Bridge の起動メッセージ（stdout 1 行目）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Hello {
    #[serde(rename = "type")]
    message_type: HelloTag,
    sdk_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum HelloTag {
    #[serde(rename = "hello")]
    Hello,
}

/// Bridge → Driver のハンドシェイク応答（stdin 1 行目）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HelloAck {
    pub compatible: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Bridge → Driver の制御コマンド（ハンドシェイク完了後の stdin）。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ControlCommand {
    /// 指定デバイスへの接続を開始する。
    Connect {
        device: String,
        #[serde(default)]
        config: serde_json::Value,
    },
    /// 現在の接続を切断する。
    Disconnect,
    /// 実行中の接続パラメータを更新する。
    Configure { config: serde_json::Value },
}

/// Bridge → Driver で受信しうる stdin メッセージの内訳。
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum BridgeMessage {
    #[serde(rename = "hello_ack")]
    HelloAck {
        compatible: bool,
        #[serde(default)]
        reason: Option<String>,
    },
    #[serde(rename = "connect")]
    Connect {
        device: String,
        #[serde(default)]
        config: serde_json::Value,
    },
    #[serde(rename = "disconnect")]
    Disconnect,
    #[serde(rename = "configure")]
    Configure { config: serde_json::Value },
}

impl BridgeMessage {
    fn into_command(self) -> Option<ControlCommand> {
        match self {
            Self::HelloAck { .. } => None,
            Self::Connect { device, config } => Some(ControlCommand::Connect { device, config }),
            Self::Disconnect => Some(ControlCommand::Disconnect),
            Self::Configure { config } => Some(ControlCommand::Configure { config }),
        }
    }
}

/// ドライバー作者が実装するハンドラ群。
///
/// すべてのメソッドは `&mut self` を取るため、ドライバー固有の状態
/// （接続ハンドル・スレッドハンドル等）を保持できる。
pub trait Driver {
    /// `<driver> list` で返すデバイス一覧。
    fn list_devices(&mut self) -> Vec<DeviceEntry>;

    /// 制御コマンドのディスパッチ先。
    /// 戻り値の `Err` は致命的とみなし、CLI を終了させる。
    fn handle_command(&mut self, command: ControlCommand) -> Result<(), DriverError>;

    /// graceful shutdown 開始時に呼ばれる。共有メモリの解放やワーカー停止を行う。
    fn shutdown(&mut self) -> Result<(), DriverError>;
}

/// ドライバー実装が返すエラー。SDK 側はメッセージを stderr に流すだけ。
#[derive(Debug)]
pub struct DriverError {
    message: String,
}

impl DriverError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DriverError {}

/// プロトコル進行中に発生したエラー。
#[derive(Debug)]
pub enum ProtocolError {
    /// stdin/stdout への書き込み・読み込みに失敗。
    Io(std::io::Error),
    /// 受信した行が JSON としてパースできなかった。
    Parse {
        line: String,
        source: serde_json::Error,
    },
    /// `hello_ack` を待っていたのに stdin が EOF した。
    HandshakeMissing,
    /// `hello_ack` の前に別メッセージが届いた。
    HandshakeOutOfOrder,
    /// Bridge から `hello_ack(compatible:false)` を受信した。
    Incompatible(Option<String>),
    /// `Driver::handle_command` / `Driver::shutdown` がエラーを返した。
    Driver(DriverError),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Parse { line, source } => {
                write!(f, "failed to parse stdin line ({source}): {line}")
            }
            Self::HandshakeMissing => f.write_str("stdin closed before hello_ack"),
            Self::HandshakeOutOfOrder => f.write_str("expected hello_ack as first stdin message"),
            Self::Incompatible(reason) => match reason {
                Some(r) => write!(f, "Bridge reported incompatibility: {r}"),
                None => f.write_str("Bridge reported incompatibility"),
            },
            Self::Driver(e) => write!(f, "driver error: {e}"),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse { source, .. } => Some(source),
            Self::Driver(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProtocolError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// `<driver> list` を実行する。pure 関数版（テスト用）。
pub fn write_device_list<W: Write, D: Driver>(driver: &mut D, out: &mut W) -> std::io::Result<()> {
    let devices = driver.list_devices();
    serde_json::to_writer(&mut *out, &devices)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// hello メッセージを 1 行で書き出す。
pub fn write_hello<W: Write>(out: &mut W, sdk_version: &str) -> std::io::Result<()> {
    let hello = Hello {
        message_type: HelloTag::Hello,
        sdk_version: sdk_version.to_owned(),
    };
    serde_json::to_writer(&mut *out, &hello)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// stdin の 1 行を [`BridgeMessage`] にパースする。
fn parse_bridge_message(line: &str) -> Result<BridgeMessage, ProtocolError> {
    serde_json::from_str(line).map_err(|source| ProtocolError::Parse {
        line: line.to_owned(),
        source,
    })
}

/// `<driver> start` のプロトコル本体。シグナル・スレッドに依存しないため、
/// テストから直接呼び出してハンドシェイクとコマンド分配を検証できる。
///
/// `lines` は stdin から 1 行ずつ供給するイテレータ。`shutdown` は外部
/// （シグナルハンドラ等）から立てるフラグで、true になればコマンド消費を
/// 停止して [`Driver::shutdown`] を呼ぶ。
pub fn run_protocol<D, I, W>(
    driver: &mut D,
    lines: I,
    out: &mut W,
    sdk_version: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<(), ProtocolError>
where
    D: Driver,
    I: IntoIterator<Item = std::io::Result<String>>,
    W: Write,
{
    write_hello(out, sdk_version)?;

    let mut iter = lines.into_iter();

    // 最初の 1 行は必ず hello_ack
    let first = iter.next().ok_or(ProtocolError::HandshakeMissing)??;
    match parse_bridge_message(&first)? {
        BridgeMessage::HelloAck {
            compatible: false,
            reason,
        } => return Err(ProtocolError::Incompatible(reason)),
        BridgeMessage::HelloAck {
            compatible: true, ..
        } => {}
        _ => return Err(ProtocolError::HandshakeOutOfOrder),
    }

    // 以降はコマンドループ
    for line in iter {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let line = line?;
        let message = parse_bridge_message(&line)?;
        if let Some(cmd) = message.into_command() {
            driver.handle_command(cmd).map_err(ProtocolError::Driver)?;
        }
        // hello_ack が再度来るのは仕様外だが、エラーにせず無視する。
    }

    driver.shutdown().map_err(ProtocolError::Driver)?;
    Ok(())
}

/// `<driver>` バイナリのエントリポイント。`fn main()` から呼び出す。
///
/// argv を見て `list` / `start` をディスパッチし、`start` ではシグナル
/// ハンドラを設定し stdin リーダースレッドを起動して [`run_protocol`] を
/// 駆動する。`sdk_version` には呼び出し元クレートで `env!("CARGO_PKG_VERSION")`
/// を渡すのが基本だが、本 SDK 自身のバージョンを埋め込みたければ
/// `midori_sdk::driver::SDK_VERSION` を使う。
pub fn run<D: Driver>(mut driver: D, sdk_version: &str) -> ExitCode {
    let mut args = std::env::args();
    let _bin = args.next();
    match args.next().as_deref() {
        Some("list") => match exec_list(&mut driver) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("midori-sdk: list failed: {err}");
                ExitCode::FAILURE
            }
        },
        Some("start") => exec_start(&mut driver, sdk_version),
        Some(other) => {
            eprintln!("midori-sdk: unknown subcommand: {other}");
            print_usage();
            ExitCode::FAILURE
        }
        None => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

/// 本 SDK クレート自身のバージョン（`<package>.version`）。
/// ドライバー作者が `run(driver, SDK_VERSION)` の形で利用できるよう公開する。
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

fn exec_list<D: Driver>(driver: &mut D) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_device_list(driver, &mut out)
}

fn exec_start<D: Driver>(driver: &mut D, sdk_version: &str) -> ExitCode {
    let shutdown = Arc::new(AtomicBool::new(false));
    if let Err(err) = register_termination_signals(&shutdown) {
        eprintln!("midori-sdk: failed to install signal handlers: {err}");
        return ExitCode::FAILURE;
    }

    let lines = stdin_lines_with_shutdown(Arc::clone(&shutdown));
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match run_protocol(driver, lines, &mut out, sdk_version, &shutdown) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("midori-sdk: protocol error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!("usage: <driver> list|start");
}

fn register_termination_signals(shutdown: &Arc<AtomicBool>) -> std::io::Result<()> {
    use signal_hook::consts::{SIGINT, SIGTERM};
    signal_hook::flag::register(SIGTERM, Arc::clone(shutdown))?;
    signal_hook::flag::register(SIGINT, Arc::clone(shutdown))?;
    Ok(())
}

/// stdin から行を読み取りつつ、`shutdown` フラグが立ったら供給を打ち切る
/// イテレータを返す。
///
/// 内部でリーダースレッドを 1 本立て、行を `mpsc` チャンネル経由で渡す。
/// チャンネル受信時は 100ms タイムアウトでポーリングし、shutdown 立っていれば
/// `None` を返す。
fn stdin_lines_with_shutdown(shutdown: Arc<AtomicBool>) -> ShutdownAwareLines {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let handle = stdin.lock();
        for line in handle.lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    ShutdownAwareLines { rx, shutdown }
}

struct ShutdownAwareLines {
    rx: mpsc::Receiver<std::io::Result<String>>,
    shutdown: Arc<AtomicBool>,
}

impl Iterator for ShutdownAwareLines {
    type Item = std::io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.shutdown.load(Ordering::Acquire) {
                return None;
            }
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => return Some(line),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct StubDriver {
        devices: Vec<DeviceEntry>,
        commands: Vec<ControlCommand>,
        shutdown_called: bool,
    }

    impl StubDriver {
        fn new(devices: Vec<DeviceEntry>) -> Self {
            Self {
                devices,
                commands: Vec::new(),
                shutdown_called: false,
            }
        }
    }

    impl Driver for StubDriver {
        fn list_devices(&mut self) -> Vec<DeviceEntry> {
            self.devices.clone()
        }

        fn handle_command(&mut self, command: ControlCommand) -> Result<(), DriverError> {
            self.commands.push(command);
            Ok(())
        }

        fn shutdown(&mut self) -> Result<(), DriverError> {
            self.shutdown_called = true;
            Ok(())
        }
    }

    fn lines_from(items: &[&str]) -> Vec<std::io::Result<String>> {
        items.iter().map(|s| Ok((*s).to_owned())).collect()
    }

    #[test]
    fn it_should_write_device_list_as_json_array() {
        let mut driver = StubDriver::new(vec![
            DeviceEntry {
                value: "ELS-03 Series".into(),
                label: "Yamaha ELS-03".into(),
            },
            DeviceEntry {
                value: "IAC Driver Bus 1".into(),
                label: "IAC Driver".into(),
            },
        ]);
        let mut out = Vec::new();
        write_device_list(&mut driver, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text.trim_end(),
            r#"[{"value":"ELS-03 Series","label":"Yamaha ELS-03"},{"value":"IAC Driver Bus 1","label":"IAC Driver"}]"#
        );
    }

    #[test]
    fn it_should_write_hello_with_sdk_version() {
        let mut out = Vec::new();
        write_hello(&mut out, "1.2.3").unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.trim_end(), r#"{"type":"hello","sdk_version":"1.2.3"}"#);
    }

    #[test]
    fn it_should_parse_compatible_hello_ack() {
        let m = parse_bridge_message(r#"{"type":"hello_ack","compatible":true}"#).unwrap();
        assert!(matches!(
            m,
            BridgeMessage::HelloAck {
                compatible: true,
                ..
            }
        ));
    }

    #[test]
    fn it_should_parse_incompatible_hello_ack_with_reason() {
        let m =
            parse_bridge_message(r#"{"type":"hello_ack","compatible":false,"reason":"too old"}"#)
                .unwrap();
        match m {
            BridgeMessage::HelloAck {
                compatible: false,
                reason,
            } => {
                assert_eq!(reason.as_deref(), Some("too old"));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn it_should_parse_connect_command() {
        let m =
            parse_bridge_message(r#"{"type":"connect","device":"ELS-03","config":{"channel":1}}"#)
                .unwrap();
        match m.into_command() {
            Some(ControlCommand::Connect { device, config }) => {
                assert_eq!(device, "ELS-03");
                assert_eq!(config["channel"], 1);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn it_should_parse_disconnect_command() {
        let m = parse_bridge_message(r#"{"type":"disconnect"}"#).unwrap();
        assert!(matches!(m.into_command(), Some(ControlCommand::Disconnect)));
    }

    #[test]
    fn it_should_run_handshake_then_dispatch_commands_then_shutdown() {
        let mut driver = StubDriver::new(vec![]);
        let mut out: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        let shutdown = Arc::new(AtomicBool::new(false));

        let lines = lines_from(&[
            r#"{"type":"hello_ack","compatible":true}"#,
            r#"{"type":"connect","device":"x"}"#,
            r#"{"type":"disconnect"}"#,
        ]);
        run_protocol(&mut driver, lines, &mut out, "1.0.0", &shutdown).unwrap();

        assert!(driver.shutdown_called);
        assert_eq!(driver.commands.len(), 2);
        assert!(matches!(driver.commands[0], ControlCommand::Connect { .. }));
        assert!(matches!(driver.commands[1], ControlCommand::Disconnect));

        // hello が 1 行目に出ていること
        let written = String::from_utf8(out.into_inner()).unwrap();
        let first_line = written.lines().next().unwrap();
        assert!(first_line.contains(r#""type":"hello""#));
        assert!(first_line.contains(r#""sdk_version":"1.0.0""#));
    }

    #[test]
    fn it_should_fail_when_hello_ack_reports_incompatible() {
        let mut driver = StubDriver::new(vec![]);
        let mut out = Vec::new();
        let shutdown = Arc::new(AtomicBool::new(false));

        let lines = lines_from(&[r#"{"type":"hello_ack","compatible":false,"reason":"too old"}"#]);
        let err = run_protocol(&mut driver, lines, &mut out, "1.0.0", &shutdown).unwrap_err();

        match err {
            ProtocolError::Incompatible(reason) => {
                assert_eq!(reason.as_deref(), Some("too old"));
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(!driver.shutdown_called);
    }

    #[test]
    fn it_should_fail_when_first_message_is_not_hello_ack() {
        let mut driver = StubDriver::new(vec![]);
        let mut out = Vec::new();
        let shutdown = Arc::new(AtomicBool::new(false));

        let lines = lines_from(&[r#"{"type":"connect","device":"x"}"#]);
        let err = run_protocol(&mut driver, lines, &mut out, "1.0.0", &shutdown).unwrap_err();
        assert!(matches!(err, ProtocolError::HandshakeOutOfOrder));
    }

    #[test]
    fn it_should_fail_when_stdin_closes_before_hello_ack() {
        let mut driver = StubDriver::new(vec![]);
        let mut out = Vec::new();
        let shutdown = Arc::new(AtomicBool::new(false));

        let lines: Vec<std::io::Result<String>> = vec![];
        let err = run_protocol(&mut driver, lines, &mut out, "1.0.0", &shutdown).unwrap_err();
        assert!(matches!(err, ProtocolError::HandshakeMissing));
    }

    #[test]
    fn it_should_stop_command_loop_when_shutdown_flag_is_set() {
        let mut driver = StubDriver::new(vec![]);
        let mut out = Vec::new();
        let shutdown = Arc::new(AtomicBool::new(true));

        // hello_ack 後すぐに shutdown が true なので、後続コマンドは消費されない
        let lines = lines_from(&[
            r#"{"type":"hello_ack","compatible":true}"#,
            r#"{"type":"connect","device":"x"}"#,
        ]);
        run_protocol(&mut driver, lines, &mut out, "1.0.0", &shutdown).unwrap();

        assert!(driver.shutdown_called);
        assert!(driver.commands.is_empty());
    }
}
