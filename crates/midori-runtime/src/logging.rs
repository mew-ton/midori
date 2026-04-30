//! Bridge 起動経路で使う中央集約 logger。`design/04-runtime-cli.md` の
//! 「ログフォーマット」表に従い、`type: log` の JSON 行（`json` 形式）または
//! 人間可読の 1 行（`text` 形式）を **stdout** に出す。Electron 側が stdout
//! pipe で受信して GUI に転送する経路と整合させるため、stderr ではなく
//! stdout を使う（spec line 33: 「ログは JSON 形式で stdout に出力する」）。
//!
//! 設計上の役割分担:
//!
//! - [`Logger`]: format / level filter を持つ純粋な値型。`render` で「出すべき
//!   行」を `Option<String>` として返す。テストはこの値型のみで完結する
//! - [`init`] / [`log`]: process-wide な `OnceLock<Logger>` を介した便利関数。
//!   `main()` の CLI parse 直後に [`init`] を一度だけ呼び、各 module からは
//!   [`log`] / [`error`] / [`warn`] / [`info`] / [`debug`] を経由してアクセス
//!
//! `type: raw-event` / `type: device-state` / `type: signal` / `type: error-path`
//! の構造化は driver spawn / SPSC ingest 配線時に追加する別経路の責務で、
//! 本 module は `type: log` のみを扱う。

use std::fmt;
use std::sync::OnceLock;

use clap::ValueEnum;

/// ログ severity。`Error` が最強で `Debug` が最弱。`--log-level <X>` のとき
/// X より弱い severity（数値が大きい側）は出力されない。
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    /// JSON / text 出力で使う小文字の文字列表現。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 出力フォーマット。`Text` は人間可読、`Json` は machine readable な 1 行 JSON。
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum LogFormat {
    Text,
    Json,
}

/// level filter と format を保持する logger 値型。`OnceLock` 経由でも値直接でも
/// 使えるよう構造体として独立させる（テストは値直接を使うことで global state
/// に依存せず並列実行できる）。
#[derive(Debug, Clone, Copy)]
pub struct Logger {
    level: LogLevel,
    format: LogFormat,
}

impl Logger {
    /// 指定した filter / format の logger を生成する。
    #[must_use]
    pub const fn new(level: LogLevel, format: LogFormat) -> Self {
        Self { level, format }
    }

    /// 1 件のログを「出すべき行」（filter を通過する場合）の文字列として返す。
    /// `None` のとき severity が filter で落とされたことを示す。caller は
    /// 戻り値をそのまま `println!` するか、テストで内容検証する。
    #[must_use]
    pub fn render(
        self,
        level: LogLevel,
        layer: &str,
        device: Option<&str>,
        message: &dyn fmt::Display,
    ) -> Option<String> {
        if level > self.level {
            return None;
        }
        let rendered = match self.format {
            LogFormat::Text => match device {
                Some(dev) => format!("midori [{level}] {layer} ({dev}): {message}"),
                None => format!("midori [{level}] {layer}: {message}"),
            },
            LogFormat::Json => render_json(level, layer, device, message),
        };
        Some(rendered)
    }
}

fn render_json(
    level: LogLevel,
    layer: &str,
    device: Option<&str>,
    message: &dyn fmt::Display,
) -> String {
    // フィールド順は `design/04-runtime-cli.md` のサンプル
    // `{"type":"log","level":...,"layer":...,"device":...,"message":...}`
    // に揃える（`device` は省略可）。
    let mut entry = serde_json::Map::new();
    entry.insert(
        "type".to_owned(),
        serde_json::Value::String("log".to_owned()),
    );
    entry.insert(
        "level".to_owned(),
        serde_json::Value::String(level.as_str().to_owned()),
    );
    entry.insert(
        "layer".to_owned(),
        serde_json::Value::String(layer.to_owned()),
    );
    if let Some(device) = device {
        entry.insert(
            "device".to_owned(),
            serde_json::Value::String(device.to_owned()),
        );
    }
    entry.insert(
        "message".to_owned(),
        serde_json::Value::String(format!("{message}")),
    );
    serde_json::Value::Object(entry).to_string()
}

/// process-wide な logger。`init` で一度設定したらそれ以降は固定。
static LOGGER: OnceLock<Logger> = OnceLock::new();

/// CLI parse 直後に 1 度だけ呼んで logger を確定する。再呼び出し（テストで
/// 連続 init するケース等）は **silent に無視**して先勝ちを採用する。
pub fn init(level: LogLevel, format: LogFormat) {
    let _ = LOGGER.set(Logger::new(level, format));
}

/// global logger を取得する。`init` 前に呼ばれた場合は default
/// (`Info` / `Text`) を返す。default が固定されるのは「`main` で init する
/// 前にログが出される経路」を防ぐ防衛的挙動で、運用上 main からは必ず init
/// される前提。
fn current() -> Logger {
    *LOGGER.get_or_init(|| Logger::new(LogLevel::Info, LogFormat::Text))
}

/// 1 件のログを **stdout** に書く（`design/04-runtime-cli.md`「ログは JSON
/// 形式で stdout に出力する」、Electron 側 stdout pipe ingestion との整合
/// のため）。filter 通過時のみ実出力する。
pub fn log(level: LogLevel, layer: &str, device: Option<&str>, message: impl fmt::Display) {
    if let Some(line) = current().render(level, layer, device, &message) {
        println!("{line}");
    }
}

/// `Error` レベルの便利関数。
pub fn error(layer: &str, device: Option<&str>, message: impl fmt::Display) {
    log(LogLevel::Error, layer, device, message);
}

/// `Warn` レベルの便利関数。
pub fn warn(layer: &str, device: Option<&str>, message: impl fmt::Display) {
    log(LogLevel::Warn, layer, device, message);
}

/// `Info` レベルの便利関数。
pub fn info(layer: &str, device: Option<&str>, message: impl fmt::Display) {
    log(LogLevel::Info, layer, device, message);
}

/// `Debug` レベルの便利関数。
#[allow(dead_code)]
pub fn debug(layer: &str, device: Option<&str>, message: impl fmt::Display) {
    log(LogLevel::Debug, layer, device, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(
        logger: Logger,
        level: LogLevel,
        layer: &str,
        device: Option<&str>,
        message: &str,
    ) -> Option<String> {
        logger.render(level, layer, device, &message)
    }

    #[test]
    fn it_should_render_text_format_with_layer_and_message() {
        let logger = Logger::new(LogLevel::Info, LogFormat::Text);
        let line = render(logger, LogLevel::Info, "bridge", None, "hello").expect("info passes");
        assert_eq!(line, "midori [info] bridge: hello");
    }

    #[test]
    fn it_should_render_text_format_with_device_segment() {
        let logger = Logger::new(LogLevel::Info, LogFormat::Text);
        let line =
            render(logger, LogLevel::Warn, "bridge", Some("midi"), "missing").expect("warn passes");
        assert_eq!(line, "midori [warn] bridge (midi): missing");
    }

    #[test]
    fn it_should_render_json_format_with_required_fields() {
        let logger = Logger::new(LogLevel::Info, LogFormat::Json);
        let line = render(logger, LogLevel::Error, "runtime", None, "boom").expect("error passes");
        let value: serde_json::Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(value["type"], "log");
        assert_eq!(value["level"], "error");
        assert_eq!(value["layer"], "runtime");
        assert_eq!(value["message"], "boom");
        // device は省略時に出ない
        assert!(
            value.get("device").is_none(),
            "device should be omitted: {line}"
        );
    }

    #[test]
    fn it_should_include_device_field_when_provided_in_json_format() {
        let logger = Logger::new(LogLevel::Info, LogFormat::Json);
        let line = render(
            logger,
            LogLevel::Info,
            "bridge",
            Some("midi"),
            "events.yaml loaded",
        )
        .expect("info passes");
        let value: serde_json::Value = serde_json::from_str(&line).expect("valid json");
        assert_eq!(value["device"], "midi");
    }

    #[test]
    fn it_should_filter_out_levels_weaker_than_threshold() {
        let logger = Logger::new(LogLevel::Warn, LogFormat::Text);
        // Warn 設定下では Info / Debug は出ない、Warn / Error は出る
        assert!(render(logger, LogLevel::Error, "x", None, "m").is_some());
        assert!(render(logger, LogLevel::Warn, "x", None, "m").is_some());
        assert!(render(logger, LogLevel::Info, "x", None, "m").is_none());
        assert!(render(logger, LogLevel::Debug, "x", None, "m").is_none());
    }

    #[test]
    fn it_should_pass_all_levels_when_filter_is_debug() {
        let logger = Logger::new(LogLevel::Debug, LogFormat::Text);
        for level in [
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
        ] {
            assert!(
                render(logger, level, "x", None, "m").is_some(),
                "debug filter should pass {level:?}"
            );
        }
    }

    #[test]
    fn it_should_only_emit_error_when_filter_is_error() {
        let logger = Logger::new(LogLevel::Error, LogFormat::Text);
        assert!(render(logger, LogLevel::Error, "x", None, "m").is_some());
        assert!(render(logger, LogLevel::Warn, "x", None, "m").is_none());
        assert!(render(logger, LogLevel::Info, "x", None, "m").is_none());
        assert!(render(logger, LogLevel::Debug, "x", None, "m").is_none());
    }

    #[test]
    fn it_should_render_log_level_as_lowercase_string() {
        assert_eq!(LogLevel::Error.as_str(), "error");
        assert_eq!(LogLevel::Warn.as_str(), "warn");
        assert_eq!(LogLevel::Info.as_str(), "info");
        assert_eq!(LogLevel::Debug.as_str(), "debug");
    }
}
