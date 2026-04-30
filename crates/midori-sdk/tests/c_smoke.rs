//! C smoke test: hosts a real C compile + link cycle of `tests/c_smoke/
//! spsc_round_trip.c`, links against `midori-sdk` as a staticlib, and runs
//! the produced binary. Exit code 0 indicates the FFI round-trip succeeded.
//!
//! C コンパイラが見つからなかった場合は warning を出して skip する
//! （ビルド全体は失敗しない）。Windows MSVC でのリンク検証は本テストの
//! 対象外で、`*-pc-windows-msvc` ターゲットでは早期 skip する。

use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn c_smoke_links_and_round_trips_spsc() {
    if cfg!(target_env = "msvc") {
        eprintln!("[c-smoke] SKIP: Windows MSVC target は本テストの対象外（README 参照）");
        return;
    }

    let Some((cc, cc_args)) = detect_c_compiler() else {
        eprintln!(
            "[c-smoke] SKIP: C コンパイラが見つかりませんでした（CC env / cc / gcc / clang のいずれも未検出）"
        );
        return;
    };

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let c_source = manifest_dir
        .join("tests")
        .join("c_smoke")
        .join("spsc_round_trip.c");
    assert!(
        c_source.is_file(),
        "C source missing: {}",
        c_source.display()
    );

    // build.rs から `cargo:rustc-env=MIDORI_SDK_HEADER_PATH=...` で渡される
    // 生成ヘッダの絶対パス。parent を `-I` ディレクトリに渡す。
    let header_path = PathBuf::from(env!("MIDORI_SDK_HEADER_PATH"));
    let header_dir = header_path
        .parent()
        .expect("header path has parent")
        .to_path_buf();
    assert!(
        header_path.is_file(),
        "generated header missing: {}",
        header_path.display()
    );

    let staticlib_path = locate_midori_sdk_staticlib();
    assert!(
        staticlib_path.is_file(),
        "midori-sdk staticlib missing: {}",
        staticlib_path.display()
    );

    // 同ホスト上で並行実行された `cargo test` が同じ binary 出力を奪い合う
    // のを防ぐため、process id を含めた専用ディレクトリに出力する（cargo
    // nextest や retry job 等の並行実行で race が起きないように）。
    let out_dir = std::env::temp_dir().join(format!("midori-sdk-c-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create out dir");
    let bin_path = out_dir.join(if cfg!(target_os = "windows") {
        "spsc_round_trip.exe"
    } else {
        "spsc_round_trip"
    });

    // Linux: staticlib のリンクには pthread / dl / m / system が必要になる
    // ことがある（Rust std crate の依存）。macOS は libSystem に統合されて
    // いるため `-framework Security` 相当が要らないケースも多い。最小構成
    // で gcc/clang に任せ、未解決シンボルが出たら都度追加する方針で書く。
    let mut cmd = Command::new(&cc);
    // `CC=cc -O2` のように env 値が args 込みのケースを尊重するため、
    // detect_c_compiler が分離した残り token を先頭に積む。
    cmd.args(&cc_args);
    cmd.arg("-O2")
        .arg("-Wall")
        .arg(format!("-I{}", header_dir.display()))
        .arg(&c_source)
        .arg(&staticlib_path)
        .arg("-o")
        .arg(&bin_path);
    if cfg!(target_os = "linux") {
        // libstd の依存。GNU ld の順序ルールで staticlib より後ろに置く。
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }

    let compile = cmd.output().expect("spawn C compiler");
    assert!(
        compile.status.success(),
        "C compile/link failed (status={:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        compile.status,
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("spawn smoke binary");
    assert!(
        run.status.success(),
        "C smoke binary exited non-zero (status={:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("[c-smoke] OK"),
        "expected OK marker, got stdout: {stdout}"
    );
}

/// `CC` env / `cc` / `gcc` / `clang` の順で C コンパイラを探す。見つから
/// なければ `None`（caller が skip する）。
///
/// `CC=cc -O2` のように引数込みで設定されている環境を尊重するため、
/// 戻り値は `(プログラム path, それ以降の args)` のペア。args は caller が
/// 自前のフラグより先に積むことで Honor される。
fn detect_c_compiler() -> Option<(PathBuf, Vec<String>)> {
    if let Ok(cc) = std::env::var("CC") {
        let mut tokens = cc.split_whitespace().map(str::to_owned);
        if let Some(prog) = tokens.next() {
            return Some((PathBuf::from(prog), tokens.collect()));
        }
    }
    for candidate in ["cc", "gcc", "clang"] {
        if which_in_path(candidate).is_some() {
            return Some((PathBuf::from(candidate), Vec::new()));
        }
    }
    None
}

/// 指定コマンドが `PATH` から見つかるかを `which`（POSIX）/ `where`
/// （Windows）でチェックする。
fn which_in_path(cmd: &str) -> Option<PathBuf> {
    let finder = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    let output = Command::new(finder).arg(cmd).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8(output.stdout).ok()?;
    let first = line.lines().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(PathBuf::from(first))
    }
}

/// `<target>/<profile>/libmidori_sdk.a`（Linux / macOS）を解決する。
/// 本テストは integration test で実行ファイル自身が
/// `<target>/<profile>/deps/c_smoke-XXX` に置かれることを利用し、その 2 階層
/// 上を target profile dir として扱う。`CARGO_TARGET_DIR` が設定されている
/// 場合も `current_exe()` 経由で正しく解決される。
fn locate_midori_sdk_staticlib() -> PathBuf {
    let test_exe = std::env::current_exe().expect("current_exe");
    // <target>/<profile>/deps/c_smoke-XXX → <target>/<profile>/
    let profile_dir = test_exe
        .parent()
        .and_then(Path::parent)
        .expect("test exe has grandparent");
    profile_dir.join(if cfg!(target_os = "windows") {
        "midori_sdk.lib"
    } else {
        "libmidori_sdk.a"
    })
}
