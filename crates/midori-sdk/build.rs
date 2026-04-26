//! ビルドスクリプト。cbindgen で C ヘッダ `midori_sdk.h` を `OUT_DIR` に生成する。
//!
//! 生成されたヘッダはテスト（`include_str!`）で内容検証され、外部 C / 他言語
//! バインディング側からは `cargo build` の `OUT_DIR` から取り出して利用する。

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let header_path = PathBuf::from(&out_dir).join("midori_sdk.h");

    let config = cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml"))
        .expect("read cbindgen.toml");

    let bindings = cbindgen::Builder::new()
        .with_config(config)
        .with_crate(&crate_dir)
        .generate()
        .expect("cbindgen failed to generate bindings");

    bindings.write_to_file(&header_path);

    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=src/spsc.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=build.rs");
}
