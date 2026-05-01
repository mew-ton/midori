//! profile YAML loader and driver-name resolver for Bridge startup.
//!
//! `midori run <profile>` で渡されるプロファイル YAML をパースし、
//! `inputs[]` / `outputs[]` の `connection.driver` から起動対象の driver 名
//! を抽出する。各 driver の `events.yaml` path 解決は本 module の責務外で、
//! caller（main）が `<app-data-dir>/plugins/driver-<name>/events.yaml` の
//! 規約で組み立てる。
//!
//! 対象スキーマは `design/config/05-profile.md` 確定版に従う。adapter YAML /
//! transform YAML の本格パースは別 subtask の責務で、本 module ではそれぞれ
//! `PathBuf` として保持するに留める。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// `profiles/*.yaml` の最上位構造。
///
/// `name` / `description` は GUI 表示用で起動経路では参照しないが、parser
/// の網羅性を担保するため受理する。`adapter` / `transform` の path は
/// 本 module ではロードしない（後続 subtask の責務）。
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileYaml {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub inputs: Vec<ProfileEndpoint>,
    pub transform: PathBuf,
    pub outputs: Vec<ProfileEndpoint>,
}

/// `inputs[]` / `outputs[]` の各エントリ。input/output は同一スキーマで
/// `direction` フィールドを持たない（spec 準拠）。
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileEndpoint {
    /// 変換グラフから参照する識別子。省略可（spec では「省略時はアダプター
    /// ファイルのベース名から自動生成」とされるが、その自動生成は別 module
    /// （変換グラフ resolver）の責務で本 module は YAML 上の値だけを保持
    /// する）。起動時 events.yaml チェックでは未使用。
    #[serde(default)]
    pub id: Option<String>,
    /// アダプター YAML への path。本 module ではロードしない。
    pub adapter: PathBuf,
    /// 実デバイスとの接続設定。`driver` キーが必須で、その他は driver 固有。
    pub connection: ProfileConnection,
}

/// `connection` セクション。`driver` 以外のフィールドは driver 固有で、
/// 本 module では解釈せず生 YAML 値として保持する。adapter 側で各 driver
/// の `connection_fields` 宣言と照合する経路は別 subtask。
#[derive(Debug, Deserialize, PartialEq)]
pub struct ProfileConnection {
    /// driver 識別子（`driver.yaml` の `name` フィールドと一致する想定）。
    pub driver: String,
    /// driver 固有の追加フィールド（`device_name` / `host` / `port` 等）。
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml_ng::Value>,
}

/// profile YAML のロード失敗。
#[derive(Debug)]
pub enum ProfileLoadError {
    /// I/O 失敗。
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// YAML パース / deserialize 失敗。
    Parse {
        path: PathBuf,
        source: serde_yaml_ng::Error,
    },
    /// パースは通ったが意味論的に invalid（inputs / outputs が空など）。
    Invalid { path: PathBuf, message: String },
}

impl std::fmt::Display for ProfileLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(
                f,
                "プロファイルの読み込みに失敗しました ({}): {source}",
                path.display()
            ),
            Self::Parse { path, source } => write!(
                f,
                "プロファイルの YAML パースに失敗しました ({}): {source}",
                path.display()
            ),
            Self::Invalid { path, message } => {
                write!(f, "プロファイル ({}) が不正です: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ProfileLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

/// `path` の profile YAML を読み込み、最低限の意味論検証まで通したものを返す。
///
/// # Errors
///
/// I/O 失敗 / YAML パース失敗 / `inputs` または `outputs` が空、のいずれかで
/// 対応する [`ProfileLoadError`] を返す。
pub fn load_from_path(path: &Path) -> Result<ProfileYaml, ProfileLoadError> {
    let yaml = fs::read_to_string(path).map_err(|source| ProfileLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let profile: ProfileYaml =
        serde_yaml_ng::from_str(&yaml).map_err(|source| ProfileLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    if profile.inputs.is_empty() {
        return Err(ProfileLoadError::Invalid {
            path: path.to_path_buf(),
            message: "inputs が空です（最低 1 件必要）".to_owned(),
        });
    }
    if profile.outputs.is_empty() {
        return Err(ProfileLoadError::Invalid {
            path: path.to_path_buf(),
            message: "outputs が空です（最低 1 件必要）".to_owned(),
        });
    }
    for endpoint in profile.inputs.iter().chain(profile.outputs.iter()) {
        validate_driver_name(&endpoint.connection.driver, path)?;
    }
    Ok(profile)
}

/// driver 名がファイルシステム経路として安全な識別子であることを検証する。
///
/// driver 名は `<app-data-dir>/plugins/driver-<name>/events.yaml` の path 構築
/// にそのまま使うため、path-traversal 攻撃（`../../etc/passwd` 等）や空白のみ
/// の値、path セパレータ含みなど、想定外の文字列を受け入れると plugins
/// ディレクトリ外を読み取ってしまう。Windows reserved characters / device
/// names もクロスプラットフォーム対応として全プラットフォームで一律 reject
/// する（POSIX 環境で書かれた profile が Windows でも安全に読めることを担保）。
fn validate_driver_name(name: &str, profile_path: &Path) -> Result<(), ProfileLoadError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ProfileLoadError::Invalid {
            path: profile_path.to_path_buf(),
            message: format!("connection.driver が空白のみの値です: `{name}`"),
        });
    }
    if trimmed != name {
        return Err(ProfileLoadError::Invalid {
            path: profile_path.to_path_buf(),
            message: format!("connection.driver の前後に空白が含まれています: `{name}`"),
        });
    }
    // POSIX 系で危険な文字（path separator / 制御文字 / 空白）+ Windows で
    // ファイル名に使えない文字（`< > : " | ? *`）を一括で弾く。
    let invalid_char = name.chars().find(|c| {
        matches!(
            c,
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        ) || c.is_control()
            || c.is_whitespace()
    });
    if let Some(c) = invalid_char {
        return Err(ProfileLoadError::Invalid {
            path: profile_path.to_path_buf(),
            message: format!("connection.driver に許容外の文字 `{c:?}` が含まれています: `{name}`"),
        });
    }
    if name == "." || name == ".." || name.contains("..") {
        return Err(ProfileLoadError::Invalid {
            path: profile_path.to_path_buf(),
            message: format!(
                "connection.driver に path traversal 文字列を含めることはできません: `{name}`"
            ),
        });
    }
    // Windows のファイル名末尾 `.` / ` ` は OS が無視するため、`midi.` /
    // `midi ` は実質 `midi` と同名のディレクトリを指す。あらかじめ拒否する。
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(ProfileLoadError::Invalid {
            path: profile_path.to_path_buf(),
            message: format!(
                "connection.driver の末尾に `.` または空白を含めることはできません: `{name}`"
            ),
        });
    }
    // Windows の予約デバイス名（拡張子の有無にかかわらず予約）。実際には
    // `driver-<name>/` と prefix が付くため Windows 上で衝突する可能性は低い
    // が、defense in depth として一律拒否する。
    if is_windows_reserved_device_name(name) {
        return Err(ProfileLoadError::Invalid {
            path: profile_path.to_path_buf(),
            message: format!("connection.driver に Windows 予約デバイス名は使えません: `{name}`"),
        });
    }
    Ok(())
}

/// Windows の予約デバイス名（`CON` / `PRN` / `AUX` / `NUL` / `COM1`〜`COM9`
/// / `LPT1`〜`LPT9`）に該当するかを大文字小文字を問わず判定する。拡張子
/// （`CON.txt` 等）が付いても予約扱いになるため、拡張子部分を除いた本体
/// （最初の `.` まで）で照合する。
fn is_windows_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(
            upper.as_str(),
            "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
        )
        || matches!(
            upper.as_str(),
            "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
        )
}

/// profile から driver 名のリストを抽出する。重複（input/output で同じ
/// driver が出る、あるいは複数 endpoint が同じ driver を使う）は最初の
/// 出現順を維持しつつ dedupe する。
#[must_use]
pub fn collect_driver_names(profile: &ProfileYaml) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for endpoint in profile.inputs.iter().chain(profile.outputs.iter()) {
        let name = endpoint.connection.driver.as_str();
        if seen.insert(name.to_owned()) {
            out.push(name.to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<ProfileYaml, ProfileLoadError> {
        // 直接 from_str を呼ばず、load_from_path 経由の分岐も含めて検査する
        // ため tempfile を経由する。
        let mut file = tempfile::Builder::new()
            .prefix("midori-profile-test-")
            .suffix(".yaml")
            .tempfile()
            .expect("tempfile");
        std::io::Write::write_all(&mut file, yaml.as_bytes()).expect("write");
        load_from_path(file.path())
    }

    const VALID_MIN: &str = r#"
inputs:
  - adapter: adapters/midi.yaml
    connection:
      driver: midi
      device_name: "ELS-03 Series"
transform: mappers/example.yaml
outputs:
  - adapter: adapters/osc.yaml
    connection:
      driver: osc
      host: 127.0.0.1
      port: 9000
"#;

    #[test]
    fn it_should_parse_minimal_valid_profile() {
        let profile = parse(VALID_MIN).expect("valid");
        assert_eq!(profile.inputs.len(), 1);
        assert_eq!(profile.outputs.len(), 1);
        assert_eq!(profile.inputs[0].connection.driver, "midi");
        assert_eq!(profile.outputs[0].connection.driver, "osc");
        assert_eq!(profile.transform, PathBuf::from("mappers/example.yaml"));
    }

    #[test]
    fn it_should_collect_driver_names_in_first_seen_order_and_dedupe() {
        let yaml = r"
inputs:
  - adapter: a.yaml
    connection: { driver: midi }
  - adapter: b.yaml
    connection: { driver: midi }
transform: t.yaml
outputs:
  - adapter: c.yaml
    connection: { driver: osc }
  - adapter: d.yaml
    connection: { driver: midi }
";
        let profile = parse(yaml).expect("valid");
        let names = collect_driver_names(&profile);
        assert_eq!(names, vec!["midi".to_owned(), "osc".to_owned()]);
    }

    #[test]
    fn it_should_reject_profile_without_inputs() {
        let yaml = r"
inputs: []
transform: t.yaml
outputs:
  - adapter: a.yaml
    connection: { driver: osc }
";
        let err = parse(yaml).expect_err("inputs empty");
        assert!(matches!(err, ProfileLoadError::Invalid { .. }));
    }

    #[test]
    fn it_should_reject_profile_without_outputs() {
        let yaml = r"
inputs:
  - adapter: a.yaml
    connection: { driver: midi }
transform: t.yaml
outputs: []
";
        let err = parse(yaml).expect_err("outputs empty");
        assert!(matches!(err, ProfileLoadError::Invalid { .. }));
    }

    #[test]
    fn it_should_reject_unknown_top_level_field() {
        let yaml = r"
inputs:
  - adapter: a.yaml
    connection: { driver: midi }
transform: t.yaml
outputs:
  - adapter: b.yaml
    connection: { driver: osc }
unknown_top: 42
";
        let err = parse(yaml).expect_err("unknown field");
        assert!(matches!(err, ProfileLoadError::Parse { .. }));
    }

    #[test]
    fn it_should_reject_endpoint_without_driver() {
        let yaml = r#"
inputs:
  - adapter: a.yaml
    connection:
      device_name: "X"
transform: t.yaml
outputs:
  - adapter: b.yaml
    connection: { driver: osc }
"#;
        let err = parse(yaml).expect_err("missing driver");
        assert!(matches!(err, ProfileLoadError::Parse { .. }));
    }

    #[test]
    fn it_should_preserve_extra_connection_fields() {
        let profile = parse(VALID_MIN).expect("valid");
        let osc = &profile.outputs[0].connection;
        assert_eq!(
            osc.extra.get("host"),
            Some(&serde_yaml_ng::Value::String("127.0.0.1".into()))
        );
        assert_eq!(
            osc.extra.get("port"),
            Some(&serde_yaml_ng::Value::Number(9000.into()))
        );
    }

    #[test]
    fn it_should_fail_when_profile_file_is_missing() {
        let err = load_from_path(Path::new("/nonexistent/midori-profile-test/profile.yaml"))
            .expect_err("missing");
        assert!(matches!(err, ProfileLoadError::Io { .. }));
    }

    #[test]
    fn it_should_reject_driver_name_with_path_traversal() {
        // `connection.driver` から `<app-data-dir>/plugins/driver-<name>/`
        // の path を作るため、`..` / path separator が入った driver 名は
        // plugins ディレクトリを脱出させ得る。`validate_driver_name` が
        // `ProfileLoadError::Invalid` として弾くことを担保する。
        // （null byte / 制御文字は YAML パーサ側で `Parse` 段階で拒否される
        // ため本テストの対象外。validate_driver_name の defensive check は
        // YAML パーサが将来仕様変更しても破綻しないための二段防御として
        // 残してある。）
        for bad in ["../etc/passwd", "midi/../../foo", "a/b", "a\\b", "..", "."] {
            let yaml = format!(
                "inputs:\n  - adapter: a.yaml\n    connection:\n      driver: \"{bad}\"\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: {{ driver: osc }}\n"
            );
            let outcome = parse(&yaml);
            assert!(
                matches!(outcome, Err(ProfileLoadError::Invalid { .. })),
                "driver `{bad}` should be rejected by validate_driver_name as Invalid, got {outcome:?}"
            );
        }
    }

    #[test]
    fn it_should_reject_whitespace_only_driver_name() {
        let yaml = "inputs:\n  - adapter: a.yaml\n    connection:\n      driver: \"   \"\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: { driver: osc }\n";
        let err = parse(yaml).expect_err("whitespace driver");
        assert!(matches!(err, ProfileLoadError::Invalid { .. }));
    }

    #[test]
    fn it_should_reject_driver_name_with_windows_reserved_characters() {
        // Windows のファイル名で許容されない文字を含む driver 名を一律 reject。
        // `"` は YAML 文字列終端と衝突するため YAML パーサ側で `Parse` エラーに
        // なるが、いずれにしても driver 名としては受理されないため両 variant
        // を許容する。
        for bad in [
            "midi:foo", "midi*x", "midi?", "midi\"q", "midi<", "midi>", "midi|x",
        ] {
            let yaml = format!(
                "inputs:\n  - adapter: a.yaml\n    connection:\n      driver: \"{bad}\"\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: {{ driver: osc }}\n"
            );
            let outcome = parse(&yaml);
            assert!(
                matches!(
                    outcome,
                    Err(ProfileLoadError::Invalid { .. } | ProfileLoadError::Parse { .. })
                ),
                "driver `{bad}` should be rejected (Windows reserved char), got {outcome:?}"
            );
        }
    }

    #[test]
    fn it_should_reject_driver_name_ending_with_dot_or_space() {
        // Windows は末尾の `.` / 空白を無視するため、`midi.` と `midi` が同一
        // ディレクトリを指してしまう。一律拒否する。
        for bad in ["midi.", "midi "] {
            let yaml = format!(
                "inputs:\n  - adapter: a.yaml\n    connection:\n      driver: \"{bad}\"\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: {{ driver: osc }}\n"
            );
            let outcome = parse(&yaml);
            assert!(
                matches!(outcome, Err(ProfileLoadError::Invalid { .. })),
                "driver `{bad}` should be rejected (trailing dot/space), got {outcome:?}"
            );
        }
    }

    #[test]
    fn it_should_reject_driver_name_matching_windows_reserved_device_name() {
        // 大文字小文字を問わず Windows 予約デバイス名を拒否する。
        for bad in [
            "con", "CON", "PRN", "Aux", "nul", "COM1", "lpt9", "CON.txt", "com5.log",
        ] {
            let yaml = format!(
                "inputs:\n  - adapter: a.yaml\n    connection:\n      driver: \"{bad}\"\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: {{ driver: osc }}\n"
            );
            let outcome = parse(&yaml);
            assert!(
                matches!(outcome, Err(ProfileLoadError::Invalid { .. })),
                "driver `{bad}` should be rejected (Windows reserved device name), got {outcome:?}"
            );
        }
    }

    #[test]
    fn it_should_reject_driver_name_with_surrounding_whitespace() {
        let yaml = "inputs:\n  - adapter: a.yaml\n    connection:\n      driver: \"  midi  \"\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: { driver: osc }\n";
        let err = parse(yaml).expect_err("padded driver");
        assert!(matches!(err, ProfileLoadError::Invalid { .. }));
    }

    #[test]
    fn it_should_render_profile_load_error_display_with_path_and_context() {
        // Invalid 経路（inputs 空）で Display に path と理由を含むことを担保。
        let yaml = "inputs: []\ntransform: t.yaml\noutputs:\n  - adapter: a.yaml\n    connection: { driver: osc }\n";
        let err = parse(yaml).expect_err("invalid");
        let rendered = err.to_string();
        assert!(
            rendered.contains("inputs が空"),
            "Display should mention violation reason, got: {rendered}"
        );
        // Io 経路（profile が存在しない）でも path を含むことを担保。
        let io_err = load_from_path(Path::new("/nonexistent/midori-profile-test/profile.yaml"))
            .expect_err("io");
        let io_rendered = io_err.to_string();
        assert!(
            io_rendered.contains("/nonexistent/"),
            "Io display should mention path, got: {io_rendered}"
        );
    }
}
