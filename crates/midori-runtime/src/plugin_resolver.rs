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
/// 各フィールドの役割:
///
/// - `driver_name`: driver.yaml の `name`（profile `connection.driver` の
///   突き合わせ用）。production code 側に caller がまだ存在しないため
///   `#[allow(dead_code)]` 付き（後続 driver spawn subtask で外す）
/// - `plugin_name`: 表示用の plugin 名（衝突レポート等の人間向け文字列）。
///   `plugin.yaml` の `name` フィールド値。複数 plugin が同じ `name` を
///   宣言できる（fallback でディレクトリ名を使うため通常は唯一）
/// - `plugin_root`: plugin の実体を特定する canonicalize 済 path。同名
///   plugin が衝突した場合に「どちらの dir を無効化すれば良いか」を診断
///   できるよう、表示名 (`plugin_name`) と分けて保持する
/// - `driver_yaml_dir`: driver.yaml が置かれている canonicalize 済
///   ディレクトリ。events.yaml もここに同居する想定
#[derive(Debug, Clone)]
pub struct ResolvedDriver {
    #[allow(dead_code)]
    pub driver_name: String,
    pub plugin_name: String,
    pub plugin_root: PathBuf,
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
/// 同 plugin 内 `drivers[]` の重複宣言も plugin 単位の malformed として
/// 扱い（warn + その plugin 全体を skip）、本 enum には現れない。
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
    ///
    /// `*_plugin` は plugin の表示名（`plugin.yaml` の `name`）、
    /// `*_plugin_path` は実体を特定するための canonicalize 済 plugin root path。
    /// 複数 plugin が同じ表示名を宣言しているケースで「どちらの dir を
    /// 無効化すれば良いか」を診断できるよう両方保持する。
    DuplicateDriver {
        driver_name: String,
        first_plugin: String,
        first_plugin_path: PathBuf,
        second_plugin: String,
        second_plugin_path: PathBuf,
    },
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
                first_plugin_path,
                second_plugin,
                second_plugin_path,
            } => write!(
                f,
                "driver `{driver_name}` が複数のプラグインから提供されています \
                 (`{first_plugin}` ({}) と `{second_plugin}` ({}))。どちらか一方を無効化してください",
                first_plugin_path.display(),
                second_plugin_path.display()
            ),
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ScanPluginsDir { source, .. } => Some(source),
            Self::DuplicateDriver { .. } => None,
        }
    }
}

/// 1 つの `drivers[]` エントリを staging に取り込む際の結果。
///
/// - `Loaded`: staging に登録された
/// - `Skipped`: その entry だけ skip（driver.yaml 不在 / malformed /
///   plugin root 外を指している等の **エントリ単位の問題**）
/// - `PluginInvalid`: plugin 全体を skip すべき（同 plugin 内 `drivers[]`
///   重複宣言など、plugin.yaml 自体の整合性問題）
#[derive(Debug)]
enum DriverEntryOutcome {
    Loaded,
    Skipped,
    PluginInvalid,
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
///
/// 同 plugin 内 `drivers[]` の **重複宣言** や **plugin root 外を指す path** は
/// plugin 単位の整合性問題として扱い、warn を出して **その plugin 全体を skip**
/// する（既に staging に登録済の他 driver も rollback される）。これは
/// module docstring の「個別 plugin の malformed は warn + skip」方針に揃える
/// ための実装で、staging map → 確定 map のマージ phase で実現する。
///
/// 別 plugin 間の同名衝突 ([`ResolveError::DuplicateDriver`]) は呼び出し元へ
/// `Err` で伝播する。
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

    let manifest: PluginManifest = match serde_yaml_ng::from_str(&plugin_yaml) {
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

    // plugin root の正規化済 path。driver.yaml の path 解決時に
    // 「正規化後も plugin root 配下に収まる」ことを担保するために使う。
    // plugin_dir 自体が canonicalize できない（broken symlink 等）ときは
    // plugin 全体を skip する。
    let plugin_root_canon = match plugin_dir.canonicalize() {
        Ok(p) => p,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "プラグインルート ({}) のキャノニカル化に失敗しました (plugin=`{plugin_name}`): {err}。このプラグインを skip します",
                    plugin_dir.display()
                ),
            );
            return Ok(());
        }
    };

    // 同 plugin 内の driver を一旦 staging に蓄積し、最後に `acc` へマージする。
    // staging を介すことで「同 plugin 内重複 / plugin root 外参照」が見つかった
    // ときに plugin 全体を rollback でき、`acc` を半端に汚染しないで済む。
    let mut staged: HashMap<String, ResolvedDriver> = HashMap::new();

    for entry in &manifest.drivers {
        match load_driver_entry_into_staged(
            &plugin_name,
            &plugin_yaml_dir,
            &plugin_root_canon,
            &entry.driver,
            &mut staged,
        ) {
            DriverEntryOutcome::Loaded | DriverEntryOutcome::Skipped => {}
            DriverEntryOutcome::PluginInvalid => {
                // この plugin 全体を skip。staging はそのまま破棄（acc に
                // マージしない）。
                return Ok(());
            }
        }
    }

    // staging を確定 map にマージ。別 plugin との同名衝突はここで fail-fast。
    for (driver_name, resolved) in staged {
        if let Some(existing) = acc.get(&driver_name) {
            return Err(ResolveError::DuplicateDriver {
                driver_name,
                first_plugin: existing.plugin_name.clone(),
                first_plugin_path: existing.plugin_root.clone(),
                second_plugin: plugin_name.clone(),
                second_plugin_path: plugin_root_canon.clone(),
            });
        }
        acc.insert(driver_name, resolved);
    }

    Ok(())
}

/// 1 つの `drivers[]` エントリ（driver.yaml の path）を staging map に
/// 取り込む。戻り値で「entry 単位の skip」「plugin 全体 skip」を分けて返す。
fn load_driver_entry_into_staged(
    plugin_name: &str,
    plugin_yaml_dir: &Path,
    plugin_root_canon: &Path,
    driver_yaml_rel: &Path,
    staged: &mut HashMap<String, ResolvedDriver>,
) -> DriverEntryOutcome {
    // 絶対パスは plugin の境界を越えうるため reject。`drivers[].driver` は
    // 必ず `plugin.yaml` 起点の相対パス（`design/10-driver-plugin.md`）。
    if driver_yaml_rel.is_absolute() {
        logging::warn(
            LOG_LAYER,
            None,
            format_args!(
                "driver.yaml の path が絶対パスです (plugin=`{plugin_name}`, path=`{}`)。`drivers[].driver` は plugin.yaml 起点の相対パスのみ許可されます。このエントリを skip します",
                driver_yaml_rel.display()
            ),
        );
        return DriverEntryOutcome::Skipped;
    }

    let driver_yaml_path = plugin_yaml_dir.join(driver_yaml_rel);

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

    // canonicalize して plugin_root 配下に収まることを確認する。`..` での脱出や
    // symlink で plugin 外を指すパスを reject するため。canonicalize はファイル
    // が存在しないと失敗するので、driver.yaml 不在ケースもここで warn + skip。
    let driver_yaml_canon = match driver_yaml_path.canonicalize() {
        Ok(p) => p,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "driver.yaml ({}) のキャノニカル化に失敗しました (plugin=`{plugin_name}`, driver_dir=`{driver_dir_label}`): {err}。このエントリを skip します",
                    driver_yaml_path.display()
                ),
            );
            return DriverEntryOutcome::Skipped;
        }
    };

    if !driver_yaml_canon.starts_with(plugin_root_canon) {
        logging::warn(
            LOG_LAYER,
            None,
            format_args!(
                "driver.yaml ({}) が plugin root ({}) の外を指しています (plugin=`{plugin_name}`)。`..` や symlink で plugin の境界を越える path は許可されません。このエントリを skip します",
                driver_yaml_canon.display(),
                plugin_root_canon.display()
            ),
        );
        return DriverEntryOutcome::Skipped;
    }

    let driver_yaml = match fs::read_to_string(&driver_yaml_canon) {
        Ok(s) => s,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "driver.yaml ({}) の読み込みに失敗しました (plugin=`{plugin_name}`, driver_dir=`{driver_dir_label}`): {err}。このエントリを skip します",
                    driver_yaml_canon.display()
                ),
            );
            return DriverEntryOutcome::Skipped;
        }
    };

    let driver: DriverManifest = match serde_yaml_ng::from_str(&driver_yaml) {
        Ok(d) => d,
        Err(err) => {
            logging::warn(
                LOG_LAYER,
                None,
                format_args!(
                    "driver.yaml ({}) のパースに失敗しました (plugin=`{plugin_name}`, driver_dir=`{driver_dir_label}`): {err}。このエントリを skip します",
                    driver_yaml_canon.display()
                ),
            );
            return DriverEntryOutcome::Skipped;
        }
    };

    // `parent()` が None になるのは「root だけ」「コンポーネントが空」のような
    // 病的ケースのみ。canonicalize 済 path では実用上起きないが、起きたとしても
    // events.yaml を file path 自身に join する形に落ちないよう、plugin_yaml_dir
    // を fallback として返しておく。
    let driver_yaml_dir = driver_yaml_canon
        .parent()
        .map_or_else(|| plugin_yaml_dir.to_path_buf(), Path::to_path_buf);

    // 同 plugin 内 `drivers[]` 重複は plugin.yaml の記述ミス。skip 方針との
    // 整合性のため、ここで早期 return せずに caller に PluginInvalid を伝え、
    // plugin 全体を rollback してもらう。
    if staged.contains_key(&driver.name) {
        logging::warn(
            LOG_LAYER,
            None,
            format_args!(
                "プラグイン `{plugin_name}` の plugin.yaml 内で driver `{driver_name}` が複数回宣言されています。このプラグイン全体を skip します",
                driver_name = driver.name
            ),
        );
        return DriverEntryOutcome::PluginInvalid;
    }

    staged.insert(
        driver.name.clone(),
        ResolvedDriver {
            driver_name: driver.name,
            plugin_name: plugin_name.to_owned(),
            plugin_root: plugin_root_canon.to_path_buf(),
            driver_yaml_dir,
        },
    );
    DriverEntryOutcome::Loaded
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
                first_plugin_path,
                second_plugin,
                second_plugin_path,
            } => {
                assert_eq!(driver_name, "midi");
                // sort 順で a-plugin が先、b-plugin が後。
                assert_eq!(first_plugin, "a-plugin");
                assert_eq!(second_plugin, "b-plugin");
                // path は canonicalize 済 (= tempdir 配下の plugin dir に一致)。
                assert!(
                    first_plugin_path.ends_with("a-plugin"),
                    "first path should point to a-plugin dir: {}",
                    first_plugin_path.display()
                );
                assert!(
                    second_plugin_path.ends_with("b-plugin"),
                    "second path should point to b-plugin dir: {}",
                    second_plugin_path.display()
                );
                // 表示名と path は別物として保持されている（同名 fallback ケースで
                // dir 名による diagnose ができることを担保）。
                assert_ne!(first_plugin_path, second_plugin_path);
            }
            other @ ResolveError::ScanPluginsDir { .. } => {
                panic!("expected DuplicateDriver, got {other:?}")
            }
        }
    }

    #[test]
    fn it_should_skip_whole_plugin_when_drivers_array_has_duplicates() {
        // 1 つの plugin.yaml が同じ driver.yaml を 2 回参照しているケース。
        // 別 plugin 間の衝突は fail-fast (`DuplicateDriver`) だが、同 plugin 内
        // の `drivers[]` 重複は plugin.yaml の記述ミスなので、その plugin 全体を
        // warn + skip する（`ResolveError` には現れない）。他に正常な plugin が
        // あれば継続できることも担保する。
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-dup-in-plugin-")
            .tempdir()
            .expect("tempdir");

        // 1 件目: 同 plugin 内重複。staging で巻き戻り、resolved には出ない
        let bad_root = tmp.path().join("plugins").join("self-dup");
        std::fs::create_dir_all(bad_root.join(".midori")).expect("mkdir");
        std::fs::write(
            bad_root.join(".midori").join("plugin.yaml"),
            "name: self-dup\n\
             drivers:\n  \
               - driver: ../drivers/midi/driver.yaml\n  \
               - driver: ../drivers/midi/driver.yaml\n",
        )
        .expect("write plugin.yaml");
        let bad_driver_dir = bad_root.join("drivers").join("midi");
        std::fs::create_dir_all(&bad_driver_dir).expect("mkdir driver");
        std::fs::write(
            bad_driver_dir.join("driver.yaml"),
            "name: midi\nmodality: midi\n",
        )
        .expect("write driver");

        // 2 件目: 別 plugin の正常 driver。plugin 単位 skip 後も resolver が
        // 続行できることを担保する。bad-plugin の midi が登録されないため
        // good-plugin の midi は衝突せず登録される。
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

        let resolved = resolve_drivers(tmp.path())
            .expect("self-duplicate must NOT fail the resolver (skip whole plugin)");
        // self-dup は丸ごと skip、good-plugin の midi だけが残る
        assert_eq!(resolved.len(), 1);
        let entry = resolved.get("midi").expect("good-plugin midi must remain");
        assert_eq!(entry.plugin_name, "good-plugin");
    }

    #[test]
    fn it_should_skip_driver_entry_with_absolute_path() {
        // `drivers[].driver` に絶対パスを渡すと、当該エントリだけ warn + skip
        // される（plugin 全体は continue する）。canonicalize 不能 / plugin
        // root 外 と並ぶ「entry 単位の skip」経路。
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-abspath-")
            .tempdir()
            .expect("tempdir");

        let plugin_root = tmp.path().join("plugins").join("abs-plugin");
        std::fs::create_dir_all(plugin_root.join(".midori")).expect("mkdir .midori");
        // 1 件目に絶対パス、2 件目に正常な相対パス。後者だけが登録される。
        let valid_driver_dir = plugin_root.join("drivers").join("midi");
        std::fs::create_dir_all(&valid_driver_dir).expect("mkdir driver");
        std::fs::write(
            valid_driver_dir.join("driver.yaml"),
            "name: midi\nmodality: midi\n",
        )
        .expect("write driver");
        std::fs::write(
            plugin_root.join(".midori").join("plugin.yaml"),
            "name: abs-plugin\n\
             drivers:\n  \
               - driver: /etc/passwd\n  \
               - driver: ../drivers/midi/driver.yaml\n",
        )
        .expect("write plugin.yaml");

        let resolved = resolve_drivers(tmp.path()).expect("absolute entry must skip, not fail");
        assert_eq!(resolved.len(), 1);
        assert!(resolved.get("midi").is_some());
    }

    #[test]
    fn it_should_skip_driver_entry_pointing_outside_plugin_root() {
        // `../` で plugin 境界を越えて別 plugin の driver.yaml を指す path は
        // canonicalize 後 plugin_root_canon 配下に収まらないため reject される。
        // 越境した entry は warn + skip、plugin 自体は他の正常 entry を続けて処理
        // できる。
        let tmp = tempfile::Builder::new()
            .prefix("midori-resolver-traversal-")
            .tempdir()
            .expect("tempdir");

        // ターゲット側 plugin: ここの driver.yaml を別 plugin から参照させる
        write_plugin(
            tmp.path(),
            "victim-plugin",
            "shared",
            "name: victim-plugin\n\
             drivers:\n  \
               - driver: ../drivers/shared/driver.yaml\n",
            Some("name: shared\nmodality: midi\n"),
            None,
        );

        // 越境元 plugin: `../../victim-plugin/drivers/shared/driver.yaml` を参照
        let evil_root = tmp.path().join("plugins").join("evil-plugin");
        std::fs::create_dir_all(evil_root.join(".midori")).expect("mkdir .midori");
        // 1 件目に越境 path、2 件目に自身の plugin 内の正常 path。後者のみ登録。
        let evil_driver_dir = evil_root.join("drivers").join("local");
        std::fs::create_dir_all(&evil_driver_dir).expect("mkdir local driver");
        std::fs::write(
            evil_driver_dir.join("driver.yaml"),
            "name: local\nmodality: midi\n",
        )
        .expect("write local driver");
        std::fs::write(
            evil_root.join(".midori").join("plugin.yaml"),
            "name: evil-plugin\n\
             drivers:\n  \
               - driver: ../../victim-plugin/drivers/shared/driver.yaml\n  \
               - driver: ../drivers/local/driver.yaml\n",
        )
        .expect("write evil plugin.yaml");

        let resolved = resolve_drivers(tmp.path()).expect("traversal must skip, not fail");
        // victim 側の `shared` と evil 側の `local` だけが登録される
        // （evil の越境 entry は skip）
        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved.get("shared").map(|d| d.plugin_name.as_str()),
            Some("victim-plugin")
        );
        assert_eq!(
            resolved.get("local").map(|d| d.plugin_name.as_str()),
            Some("evil-plugin")
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
    fn it_should_render_duplicate_driver_error_with_both_plugin_names_and_paths() {
        // 複数 plugin が同じ表示名を宣言しているケースで、dir path が出力に
        // 含まれていれば「どちらの実体を無効化すれば良いか」を診断できる。
        let err = ResolveError::DuplicateDriver {
            driver_name: "midi".to_owned(),
            first_plugin: "shared-name".to_owned(),
            first_plugin_path: PathBuf::from("/opt/midori/plugins/installed-a"),
            second_plugin: "shared-name".to_owned(),
            second_plugin_path: PathBuf::from("/opt/midori/plugins/installed-b"),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("midi"), "got: {rendered}");
        assert!(rendered.contains("`shared-name`"), "got: {rendered}");
        // 表示名が同じでも path で区別がつくこと
        assert!(rendered.contains("installed-a"), "got: {rendered}");
        assert!(rendered.contains("installed-b"), "got: {rendered}");
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
