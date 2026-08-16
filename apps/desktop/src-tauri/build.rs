/** 从根 package.json 读取统一版本号，注入 OUT_DIR/version.txt（lib.rs 使用）。 */

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    tauri_build::build();
    // 仓库根：<apps/desktop/src-tauri>/../../../package.json
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("package.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("无法读取根 package.json（{}）: {error}", manifest.display()));
    // 手写轻量解析 "version": "..." 字段（避免在 build-dependencies 引入 serde_json）。
    let version = text
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("\"version\":")?;
            let value = rest.trim().trim_start_matches('"');
            let end = value.find('"')?;
            Some(&value[..end])
        })
        .unwrap_or_else(|| panic!("根 package.json 缺少 version 字段"));
    if !version.chars().all(|ch| ch.is_ascii() && (ch.is_ascii_alphanumeric() || ".-+".contains(ch))) {
        panic!("根 package.json version 含有非法字符: {version:?}");
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR 未设置")).join("version.txt");
    fs::write(&out, version).expect("写入 version.txt 失败");
}
