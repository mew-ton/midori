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
    /// 変換グラフから参照する識別子。省略時はアダプターファイルのベース名。
    /// 起動時 events.yaml チェックでは未使用。
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
    pub extra: BTreeMap<String, serde_yml::Value>,
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
        source: serde_yml::Error,
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
        serde_yml::from_str(&yaml).map_err(|source| ProfileLoadError::Parse {
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
    Ok(profile)
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
            .prefix("midori-mew55-profile-")
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
            Some(&serde_yml::Value::String("127.0.0.1".into()))
        );
        assert_eq!(
            osc.extra.get("port"),
            Some(&serde_yml::Value::Number(9000.into()))
        );
    }

    #[test]
    fn it_should_fail_when_profile_file_is_missing() {
        let err = load_from_path(Path::new("/nonexistent/midori-mew55/profile.yaml"))
            .expect_err("missing");
        assert!(matches!(err, ProfileLoadError::Io { .. }));
    }
}
