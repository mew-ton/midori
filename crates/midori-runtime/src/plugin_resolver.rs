//! `<app-data-dir>/plugins/*/.midori/plugin.yaml` ベースの driver manifest
//! resolver。
//!
//! 旧経路は `<app-data-dir>/plugins/driver-<name>/events.yaml` という暫定の
//! ハードコード規約だったが、本 module は `design/09-plugin.md` /
//! `design/10-driver-plugin.md` に従い:
//!
//! - `<app-data-dir>/plugins/<plugin-name>/.midori/plugin.yaml` をプラグイン
//!   マニフェストとしてロードする
//! - その `drivers[].driver` 相対パスを `plugin.yaml` ディレクトリから解決し、
//!   各 `driver.yaml` の `name` フィールドを driver 識別子として採用する
//! - `events.yaml` は `driver.yaml` の隣に置かれている前提で path を組む
//!
//! 主要エントリは [`resolve_drivers`]。caller（main）は profile から得た
//! driver 名を引数に [`ResolvedDrivers::get`] / [`ResolvedDrivers::events_yaml_path_for`]
//! で events.yaml path を引く。`events_yaml_path_for` は driver process spawn
//! 経路（後続 subtask）から直接呼ぶ前提で `pub` のまま残している。
//!
//! 失敗ポリシー:
//!
//! - **同名 driver の衝突** → [`ResolveError::DuplicateDriver`] で fail-fast
//!   （どのプラグインの driver が起動するか曖昧になるため）
//! - **個別 plugin.yaml / driver.yaml の malformed / 不在** → 当該 plugin を
//!   skip して warn ログを出し、他の plugin は継続 (`logging::warn`)
//! - **plugins root の I/O 失敗** → [`ResolveError::ScanPluginsDir`]
//!   （ディレクトリ自体が読めないと resolver の前提が崩れるため）

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::logging;

/// log layer 識別子。`design/00-naming.md` の表に従い `bridge` を使う
/// （プロファイル読込・events.yaml 起動チェック等と同列の汎用ログ）。
const LOG_LAYER: &str = "bridge";

/// `.midori/plugin.yaml` の Rust 表現。本 module は `name` と `drivers[]` の
/// `driver` path だけを参照する。`adapter_kinds` / `render_components` などの
/// 他フィールドは将来別 module の責務になるため、ここでは serde の
/// `deny_unknown_fields` を **使わず** 受理し、将来の拡張で破綻しないよう
/// にしておく。
#[derive(Debug, Deserialize)]
struct PluginManifest {
    /// プラグイン識別子（`<app-data-dir>/plugins/<name>/` のディレクトリ名と
    /// 一致する想定だが、本 resolver では衝突レポート時の表示にのみ使用する）。
    #[serde(default)]
    name: Option<String>,
    /// 同梱するドライバーの宣言。各エントリは `driver.yaml` への
    /// `plugin.yaml` 起点の相対パスを持つ。
    #[serde(default)]
    drivers: Vec<PluginDriverEntry>,
}

#[derive(Debug, Deserialize)]
struct PluginDriverEntry {
    driver: PathBuf,
}

/// `driver.yaml` の Rust 表現。本 module は `name` フィールドだけを参照する
/// （`events.yaml` 解決の鍵になる）。`modality` / `release_assets` などの
/// 他フィールドは driver process spawn 経路の責務で、本 resolver の
/// スコープ外。`deny_unknown_fields` は付けない。
#[derive(Debug, Deserialize)]
struct DriverManifest {
    name: String,
}

/// 1 件の解決済みドライバー。`driver_yaml_dir` は events.yaml が同居する
/// ディレクトリで、caller は [`ResolvedDriver::events_yaml_path`] で events.yaml
/// path を取得する。
///
/// `driver_name` / `plugin_name` はどちらも driver process spawn 経路（別
/// subtask）で binary path 解決や診断メッセージに使う想定で公開フィールド
/// として残しているが、`#[allow(dead_code)]` の付き方が両者で異なる:
/// `plugin_name` は本 module 内 ([`ResolveError::DuplicateDriver`] の構築)
/// で参照されるため production build でも warning が出ず attribute 不要、
/// `driver_name` は production code 側に caller がまだ存在しないため
/// attribute で suppress している。
#[derive(Debug, Clone)]
pub struct ResolvedDriver {
    /// driver.yaml の `name` フィールド値。profile の `connection.driver` と
    /// 突き合わせる。production code 側からはまだ参照されないため
    /// `#[allow(dead_code)]` 付き（後続 driver spawn subtask で外す）。
    #[allow(dead_code)]
    pub driver_name: String,
    /// この driver を提供するプラグイン名（衝突レポート等で表示）。
    pub plugin_name: String,
    /// driver.yaml が置かれているディレクトリ。events.yaml もここにある想定。
    pub driver_yaml_dir: PathBuf,
}

impl ResolvedDriver {
    /// `<driver_yaml_dir>/events.yaml` を返す。
    #[must_use]
    pub fn events_yaml_path(&self) -> PathBuf {
        self.driver_yaml_dir.join("events.yaml")
    }
}

/// `<app-data-dir>/plugins/` を走査して得た driver name → manifest のマップ。
#[derive(Debug, Default)]
pub struct ResolvedDrivers {
    drivers: HashMap<String, ResolvedDriver>,
}

impl ResolvedDrivers {
    /// driver 名から解決済みエントリを引く。未登録なら `None`。
    #[must_use]
    pub fn get(&self, driver_name: &str) -> Option<&ResolvedDriver> {
        self.drivers.get(driver_name)
    }

    /// driver 名に対応する events.yaml path を返す。未登録なら `None`。
    /// caller が `get(...).map(...)` を毎回書かなくて済むよう用意したショート
    /// カット。現在は単体テストのみで使用しているが、driver spawn 経路から
    /// 直接呼ばれる予定で公開している。
    #[allow(dead_code)]
    #[must_use]
    pub fn events_yaml_path_for(&self, driver_name: &str) -> Option<PathBuf> {
        self.get(driver_name).map(ResolvedDriver::events_yaml_path)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.drivers.len()
    }
}

/// resolver の失敗種別。**個別 plugin の malformed は失敗にしない**
/// （warn ログを出して skip する）ため、本 enum には含まれない。
#[derive(Debug)]
pub enum ResolveError {
    /// `<app-data-dir>/plugins/` 自体の I/O 失敗。directory listing が
    /// 取れないと resolver は機能できない。
    ScanPluginsDir {
        path: PathBuf,
        source: std::io::Error,
    },
    /// 同名の driver が **異なる** plugin から複数提供されている。どちらを
    /// 起動すべきか曖昧なので fail-fast する。
    DuplicateDriver {
        driver_name: String,
        first_plugin: String,
        second_plugin: String,
    },
    /// 同名の driver が **同じ plugin の `drivers[]` 配列内**で重複宣言
    /// されている。これは plugin.yaml 自体の記述ミスで、上記の plugin 間
    /// 衝突とは原因も対処も違うため別 variant にして明示する。
    DuplicateDriverInPlugin { driver_name: String, plugin: String },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScanPluginsDir { path, source } => write!(
                f,
                "プラグインディレクトリ ({}) の走査に失敗しました: {source}",
                path.display()
            ),
            Self::DuplicateDriver {
                driver_name,
                first_plugin,
                second_plugin,
            } => write!(
                f,
                "driver `{driver_name}` が複数のプラグインから提供されています \
                 (`{first_plugin}` と `{second_plugin}`)。どちらか一方を無効化してください"
            ),
            Self::DuplicateDriverInPlugin {
                driver_name,
                plugin,
            } => write!(
                f,
                "プラグイン `{plugin}` の plugin.yaml 内で driver `{driver_name}` が \
                 複数回宣言されています。同じ driver を二度書かないよう plugin.yaml を修正してください"
            ),
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ScanPluginsDir { source, .. } => Some(source),
            Self::DuplicateDriver { .. } | Self::DuplicateDriverInPlugin { .. } => None,
        }
    }
}

/// `<app-data-dir>/plugins/` 配下を走査して driver manifest 群を解決する。
///
/// `plugins/` ディレクトリ自体が存在しない場合は空の [`ResolvedDrivers`]
/// を返す（plugin が 1 件もインストールされていない初期状態を許容する）。
///
/// # Errors
///
/// - `<app-data-dir>/plugins/` の listing が I/O エラーで取れない
///   → [`ResolveError::ScanPluginsDir`]
/// - 同名 driver の衝突 → [`ResolveError::DuplicateDriver`]
///
/// 個別 plugin の malformed / 不在は warn ログを出して skip する
/// （本関数の戻り値には現れない）。
pub fn resolve_drivers(app_data_dir: &Path) -> Result<ResolvedDrivers, ResolveError> {
    let plugins_root = app_data_dir.join("plugins");
    let entries = match fs::read_dir(&plugins_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // plugin 未インストール状態。空のマップを返す。
            return Ok(ResolvedDrivers::default());
        }
        Err(source) => {
            return Err(ResolveError::ScanPluginsDir {
                path: plugins_root,
                source,
            });
        }
    };

    let mut resolved: HashMap<String, ResolvedDriver> = HashMap::new();
    // 出現順を安定させるため plugin ディレクトリ名でソートする。read_dir の
    // 戻り順は OS 依存で、衝突レポートが flaky になるのを避ける。
    let mut plugin_dirs: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(source) => {
                return Err(ResolveError::ScanPluginsDir {
                    path: plugins_root.clone(),
                    source,
                });
            }
        };
        let path = entry.path();
        // `<app-data-dir>/plugins/<name>/` のディレクトリのみ対象。symlink は
        // 開発時のローカルプラグイン経路（`design/09-plugin.md` 参照）で
        // 使われるため follow する（`metadata` は symlink を辿る）。
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(err) => {
                logging::warn(
                    LOG_LAYER,
                    None,
                    format_args!(
                        "プラグインエントリ ({}) のメタデータ取得に失敗しました: {err}。skip します",
                        path.display()
                    ),
                );
                continue;
            }
        };
        if !metadata.is_dir() {
            continue;
        }
        plugin_dirs.push(path);
    }
    plugin_dirs.sort();

    for plugin_dir in plugin_dirs {
        load_plugin_into(&plugin_dir, &mut resolved)?;
    }

    Ok(ResolvedDrivers { drivers: resolved })
}

/// 1 plugin ディレクトリを処理し、解決できた driver を `acc` に追記する。
/// plugin.yaml / driver.yaml の不在 / malformed は warn ログを出して skip
/// するが、衝突は呼び出し元へ `ResolveError` で伝播する。
fn load_plugin_into(
    plugin_dir: &Path,
    acc: &mut HashMap<String, ResolvedDriver>,
) -> Result<(), ResolveError> {
    let plugin_yaml_path = plugin_dir.join(".midori").join("plugin.yaml");
    let plugin_yaml = match fs::read_to_string(&plugin_yaml_path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // `.midori/plugin.yaml` の無いディレクトリは midori 用ではない
            // とみなして silent skip。debug ログ相当だが、warn で残すほどの
            // 異常ではないので何も出さない。
            return Ok(());
        }
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "プラグインマニフェスト ({}) の読み込みに失敗しました: {err}。このプラグインを skip します",
                    plugin_yaml_path.display()
                ),
            );
            return Ok(());
        }
    };

    let manifest: PluginManifest = match serde_yml::from_str(&plugin_yaml) {
        Ok(m) => m,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "プラグインマニフェスト ({}) のパースに失敗しました: {err}。このプラグインを skip します",
                    plugin_yaml_path.display()
                ),
            );
            return Ok(());
        }
    };

    // plugin 名の表示用 fallback: `name` が無ければディレクトリ名を使う
    // （path-traversal 検査は行わない。本 resolver は表示専用に名前を扱う
    // だけで、ファイルアクセスは driver path 経由で行うため）。
    let plugin_name = manifest
        .name
        .clone()
        .or_else(|| {
            plugin_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| plugin_yaml_path.display().to_string());

    let plugin_yaml_dir = plugin_yaml_path
        .parent()
        .map_or_else(|| plugin_dir.join(".midori"), Path::to_path_buf);

    for entry in &manifest.drivers {
        load_driver_entry_into(&plugin_name, &plugin_yaml_dir, &entry.driver, acc)?;
    }

    Ok(())
}

/// 1 つの `drivers[]` エントリ（driver.yaml の path）を処理する。
fn load_driver_entry_into(
    plugin_name: &str,
    plugin_yaml_dir: &Path,
    driver_yaml_rel: &Path,
    acc: &mut HashMap<String, ResolvedDriver>,
) -> Result<(), ResolveError> {
    // `plugin.yaml` 起点の相対パスを解決。絶対パスが渡されたらそのまま使う。
    let driver_yaml_path = if driver_yaml_rel.is_absolute() {
        driver_yaml_rel.to_path_buf()
    } else {
        plugin_yaml_dir.join(driver_yaml_rel)
    };

    // log メッセージ用の driver ディレクトリ名（`<plugin>/drivers/<name>/driver.yaml`
    // の `<name>` 部）。driver.yaml の `name` フィールドが読めない段階の warn に
    // traceability を与えるため、慣例的なディレクトリ名を message に含める。
    // `design/00-naming.md` の `device` は driver.yaml の `name` を入れる枠なので、
    // 識別前のこの段階では `device=None` のまま、message 本文側で補う。
    let driver_dir_label = driver_yaml_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        .unwrap_or("?");

    let driver_yaml = match fs::read_to_string(&driver_yaml_path) {
        Ok(s) => s,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "driver.yaml ({}) の読み込みに失敗しました (plugin=`{plugin_name}`, driver_dir=`{driver_dir_label}`): {err}。このエントリを skip します",
                    driver_yaml_path.display()
                ),
            );
            return Ok(());
        }
    };

    let driver: DriverManifest = match serde_yml::from_str(&driver_yaml) {
        Ok(d) => d,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "driver.yaml ({}) のパースに失敗しました (plugin=`{plugin_name}`, driver_dir=`{driver_dir_label}`): {err}。このエントリを skip します",
                    driver_yaml_path.display()
                ),
            );
            return Ok(());
        }
    };

    // `parent()` が None になるのは「root だけ」「コンポーネントが空」のような
    // 病的ケースのみ。`plugin_yaml_dir.join(driver_yaml_rel)` の結果では実用上
    // 起きないが、起きたとしても events.yaml を file path 自身に join する形に
    // 落ちないよう、plugin_yaml_dir を fallback として返しておく。
    let driver_yaml_dir = driver_yaml_path
        .parent()
        .map_or_else(|| plugin_yaml_dir.to_path_buf(), Path::to_path_buf);

    if let Some(existing) = acc.get(&driver.name) {
        // 同 plugin 内 `drivers[]` の重複宣言と、別 plugin 間の衝突は
        // 原因が違うので別 variant にして区別する。前者は plugin.yaml の
        // 記述ミス、後者はインストール構成上の衝突。
        if existing.plugin_name == plugin_name {
            return Err(ResolveError::DuplicateDriverInPlugin {
                driver_name: driver.name,
                plugin: plugin_name.to_owned(),
            });
        }
        return Err(ResolveError::DuplicateDriver {
            driver_name: driver.name,
            first_plugin: existing.plugin_name.clone(),
            second_plugin: plugin_name.to_owned(),
        });
    }

    acc.insert(
        driver.name.clone(),
        ResolvedDriver {
            driver_name: driver.name,
            plugin_name: plugin_name.to_owned(),
            driver_yaml_dir,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    /// テスト用にプラグイン三層構造を組み立てるヘルパ。
    ///
    /// `plugin_yaml` / `driver_yaml` / `events_yaml` をそれぞれ
    /// `<app-data-dir>/plugins/<plugin>/.midori/plugin.yaml` /
    /// `<app-data-dir>/plugins/<plugin>/drivers/<driver_subdir>/driver.yaml` /
    /// 同 `events.yaml` に配置する。
    fn write_plugin(
        app_data_dir: &Path,
        plugin: &str,
        driver_subdir: &str,
        plugin_yaml: &str,
        driver_yaml: Option<&str>,
        events_yaml: Option<&str>,
    ) {
        let plugin_root = app_data_dir.join("plugins").join(plugin);
        let dot_midori = plugin_root.join(".midori");
        std::fs::create_dir_all(&dot_midori).expect("mkdir .midori");
        let mut f = std::fs::File::create(dot_midori.join("plugin.yaml")).expect("plugin.yaml");
        f.write_all(plugin_yaml.as_bytes()).expect("write");

        if let Some(driver_yaml) = driver_yaml {
            let driver_dir = plugin_root.join("drivers").join(driver_subdir);
            std::fs::create_dir_all(&driver_dir).expect("mkdir driver");
            let mut f = std::fs::File::create(driver_dir.join("driver.yaml")).expect("driver.yaml");
            f.write_all(driver_yaml.as_bytes()).expect("write driver");
            if let Some(events_yaml) = events_yaml {
                let mut f =
                    std::fs::File::create(driver_dir.join("events.yaml")).expect("events.yaml");
                f.write_all(events_yaml.as_bytes()).expect("write events");
            }
        }
    }

    #[test]
    fn it_should_resolve_three_layer_layout_into_driver_map() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-test-")
            .tempdir()
            .expect("tempdir");

        write_plugin(
            tmp.path(),
            "midi-plugin",
            "midi",
            "name: midi-plugin\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n",
            Some("name: midi\nmodality: midi\n"),
            Some("schema_version: 1\nevents: {}\n"),
        );

        let resolved = resolve_drivers(tmp.path()).expect("resolve");
        let entry = resolved.get("midi").expect("midi entry");
        assert_eq!(entry.driver_name, "midi");
        assert_eq!(entry.plugin_name, "midi-plugin");
        let events = entry.events_yaml_path();
        assert!(events.ends_with("events.yaml"), "got {}", events.display());
        assert!(
            events.exists(),
            "events.yaml must be reachable: {}",
            events.display()
        );

        // events_yaml_path_for は同じ値を返す。
        assert_eq!(resolved.events_yaml_path_for("midi"), Some(events));
        // 未登録 driver は None。
        assert!(resolved.get("absent").is_none());
        assert!(resolved.events_yaml_path_for("absent").is_none());
    }

    #[test]
    fn it_should_return_empty_when_plugins_dir_is_absent() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-empty-")
            .tempdir()
            .expect("tempdir");
        // plugins/ を作らない
        let resolved = resolve_drivers(tmp.path()).expect("resolve");
        assert_eq!(resolved.len(), 0);
    }

    #[test]
    fn it_should_resolve_multiple_plugins() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-multi-")
            .tempdir()
            .expect("tempdir");

        write_plugin(
            tmp.path(),
            "midi-plugin",
            "midi",
            "name: midi-plugin\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n",
            Some("name: midi\nmodality: midi\n"),
            None,
        );
        write_plugin(
            tmp.path(),
            "osc-plugin",
            "osc",
            "name: osc-plugin\n\
             drivers:\n  \
               - driver: ../drivers/osc/driver.yaml\n",
            Some("name: osc\nmodality: osc\n"),
            None,
        );

        let resolved = resolve_drivers(tmp.path()).expect("resolve");
        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved.get("midi").map(|d| d.plugin_name.as_str()),
            Some("midi-plugin")
        );
        assert_eq!(
            resolved.get("osc").map(|d| d.plugin_name.as_str()),
            Some("osc-plugin")
        );
    }

    #[test]
    fn it_should_detect_duplicate_driver_across_plugins() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-dup-")
            .tempdir()
            .expect("tempdir");

        write_plugin(
            tmp.path(),
            "a-plugin",
            "midi",
            "name: a-plugin\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n",
            Some("name: midi\nmodality: midi\n"),
            None,
        );
        write_plugin(
            tmp.path(),
            "b-plugin",
            "midi",
            "name: b-plugin\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n",
            Some("name: midi\nmodality: midi\n"),
            None,
        );

        let err = resolve_drivers(tmp.path()).expect_err("collision must error");
        match err {
            ResolveError::DuplicateDriver {
                driver_name,
                first_plugin,
                second_plugin,
            } => {
                assert_eq!(driver_name, "midi");
                // sort 順で a-plugin が先、b-plugin が後。
                assert_eq!(first_plugin, "a-plugin");
                assert_eq!(second_plugin, "b-plugin");
            }
            other @ (ResolveError::ScanPluginsDir { .. }
            | ResolveError::DuplicateDriverInPlugin { .. }) => {
                panic!("expected DuplicateDriver, got {other:?}")
            }
        }
    }

    #[test]
    fn it_should_detect_duplicate_driver_within_single_plugin() {
        // 1 つの plugin.yaml が同じ driver.yaml を 2 回参照しているケース。
        // 別 plugin 間の衝突 (`DuplicateDriver`) ではなく、plugin.yaml 自身の
        // 記述ミスとして `DuplicateDriverInPlugin` で報告されること。
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-dup-in-plugin-")
            .tempdir()
            .expect("tempdir");

        let plugin_root = tmp.path().join("plugins").join("self-dup");
        std::fs::create_dir_all(plugin_root.join(".midori")).expect("mkdir");
        std::fs::write(
            plugin_root.join(".midori").join("plugin.yaml"),
            "name: self-dup\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n  \
               - driver: ../drivers/midi/driver.yaml\n",
        )
        .expect("write plugin.yaml");
        let driver_dir = plugin_root.join("drivers").join("midi");
        std::fs::create_dir_all(&driver_dir).expect("mkdir driver");
        std::fs::write(
            driver_dir.join("driver.yaml"),
            "name: midi\nmodality: midi\n",
        )
        .expect("write driver");

        let err = resolve_drivers(tmp.path()).expect_err("self-duplicate must error");
        match err {
            ResolveError::DuplicateDriverInPlugin {
                driver_name,
                plugin,
            } => {
                assert_eq!(driver_name, "midi");
                assert_eq!(plugin, "self-dup");
            }
            other
            @ (ResolveError::DuplicateDriver { .. } | ResolveError::ScanPluginsDir { .. }) => {
                panic!("expected DuplicateDriverInPlugin, got {other:?}")
            }
        }
    }

    #[test]
    fn it_should_render_duplicate_driver_in_plugin_error_with_single_plugin_name() {
        // `DuplicateDriver` と表示が紛れないこと（誤解を招く「X と X」の旧挙動が
        // 出ていないこと）を Display レイヤで担保する。
        let err = ResolveError::DuplicateDriverInPlugin {
            driver_name: "midi".to_owned(),
            plugin: "self-dup".to_owned(),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("midi"), "got: {rendered}");
        assert!(rendered.contains("`self-dup`"), "got: {rendered}");
        // 「X と X」の重複表記が出ていないこと（旧 `DuplicateDriver` の文言を流用
        // していないこと）。
        assert!(
            !rendered.contains("`self-dup` と `self-dup`"),
            "should not look like cross-plugin collision: {rendered}"
        );
    }

    #[test]
    fn it_should_skip_malformed_plugin_yaml_and_continue_others() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-malformed-")
            .tempdir()
            .expect("tempdir");

        // 1 件目: malformed plugin.yaml（YAML 構文エラー）
        write_plugin(
            tmp.path(),
            "bad-plugin",
            "x",
            "name: bad\ndrivers: [\n", // `[` 閉じない
            None,
            None,
        );
        // 2 件目: 正常
        write_plugin(
            tmp.path(),
            "good-plugin",
            "midi",
            "name: good-plugin\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n",
            Some("name: midi\nmodality: midi\n"),
            None,
        );

        let resolved = resolve_drivers(tmp.path()).expect("malformed must be skipped, not fail");
        // bad-plugin の driver は登録されないが、good-plugin は通る。
        assert_eq!(resolved.len(), 1);
        assert!(resolved.get("midi").is_some());
    }

    #[test]
    fn it_should_skip_plugin_dir_without_dotmidori_manifest_silently() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-nomanifest-")
            .tempdir()
            .expect("tempdir");
        // plugins/orphan/ だけ作って .midori/plugin.yaml を置かない
        std::fs::create_dir_all(tmp.path().join("plugins").join("orphan")).expect("mkdir");
        // 別に正常 plugin を 1 件
        write_plugin(
            tmp.path(),
            "good-plugin",
            "midi",
            "name: good-plugin\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n",
            Some("name: midi\nmodality: midi\n"),
            None,
        );
        let resolved = resolve_drivers(tmp.path()).expect("orphan dir tolerated");
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn it_should_skip_driver_entry_when_driver_yaml_missing() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-nodriver-")
            .tempdir()
            .expect("tempdir");
        // plugin.yaml だけ存在し、driver.yaml の指すパスにファイルが無い
        let plugin_root = tmp.path().join("plugins").join("partial");
        std::fs::create_dir_all(plugin_root.join(".midori")).expect("mkdir");
        std::fs::write(
            plugin_root.join(".midori").join("plugin.yaml"),
            "name: partial\ndrivers:\n  - driver: ../drivers/missing/driver.yaml\n",
        )
        .expect("write plugin.yaml");

        let resolved = resolve_drivers(tmp.path()).expect("missing driver.yaml must be skipped");
        assert_eq!(resolved.len(), 0);
    }

    #[test]
    fn it_should_skip_malformed_driver_yaml_but_keep_other_drivers_in_same_plugin() {
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-mixed-driver-")
            .tempdir()
            .expect("tempdir");

        let plugin_root = tmp.path().join("plugins").join("mixed");
        std::fs::create_dir_all(plugin_root.join(".midori")).expect("mkdir");
        std::fs::write(
            plugin_root.join(".midori").join("plugin.yaml"),
            "name: mixed\n\
             drivers:\n  \
               - driver: ../drivers/bad/driver.yaml\n  \
               - driver: ../drivers/midi/driver.yaml\n",
        )
        .expect("write plugin.yaml");

        let bad_dir = plugin_root.join("drivers").join("bad");
        std::fs::create_dir_all(&bad_dir).expect("mkdir bad");
        // `name:` フィールドが欠けている（DriverManifest デシリアライズ失敗）
        std::fs::write(bad_dir.join("driver.yaml"), "modality: foo\n").expect("write bad");

        let good_dir = plugin_root.join("drivers").join("midi");
        std::fs::create_dir_all(&good_dir).expect("mkdir good");
        std::fs::write(good_dir.join("driver.yaml"), "name: midi\nmodality: midi\n")
            .expect("write good");

        let resolved = resolve_drivers(tmp.path()).expect("malformed driver yaml is skipped");
        assert_eq!(resolved.len(), 1);
        assert!(resolved.get("midi").is_some());
    }

    #[test]
    fn it_should_render_duplicate_driver_error_with_both_plugin_names() {
        let err = ResolveError::DuplicateDriver {
            driver_name: "midi".to_owned(),
            first_plugin: "a".to_owned(),
            second_plugin: "b".to_owned(),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("midi"), "got: {rendered}");
        assert!(rendered.contains("`a`"), "got: {rendered}");
        assert!(rendered.contains("`b`"), "got: {rendered}");
    }

    #[test]
    fn it_should_render_scan_error_display_with_path() {
        let err = ResolveError::ScanPluginsDir {
            path: PathBuf::from("/nonexistent/midori-test/plugins"),
            source: std::io::Error::other("boom"),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("/nonexistent/"), "got: {rendered}");
        assert!(rendered.contains("boom"), "got: {rendered}");
    }
}
