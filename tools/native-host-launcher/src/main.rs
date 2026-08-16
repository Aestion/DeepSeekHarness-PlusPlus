// DSH++ Chrome native messaging host launcher.
//
// Chrome launches the host binary listed in the native messaging host
// manifest with stdio pipes attached. A .cmd/.bat wrapper is unreliable
// across Chrome versions (CreateProcess does not run batch files), so this
// small executable forwards stdio to the real Node host script instead.
//
// Layout expectations (portable bundle):
//   <stage>/.portable/browser-extension/native-host-launcher.exe
//   <stage>/runtime/node/node.exe
//   <stage>/.portable/browser-extension/native-host.mjs
// The launcher resolves both paths relative to its own location, so the
// bundle can live anywhere on disk.

// GUI subsystem: no console window pops up when Chrome spawns the host.
// stdio pipes are inherited regardless of subsystem.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::{Command, Stdio};

const GATEWAY: &str = "http://127.0.0.1:18766";

fn main() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    // exe lives at <stage>/.portable/browser-extension/; node at <stage>/runtime/node/node.exe
    let node = exe_dir.join("../../runtime/node/node.exe");
    let script = exe_dir.join("native-host.mjs");

    if !node.is_file() || !script.is_file() {
        eprintln!(
            "native-host-launcher: missing node ({}) or script ({})",
            node.display(),
            script.display()
        );
        std::process::exit(1);
    }

    let status = Command::new(&node)
        .arg(&script)
        .env("DSHPLUSPLUS_GATEWAY", GATEWAY)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .and_then(|mut child| child.wait())
        .map(|status| status.code().unwrap_or(1))
        .unwrap_or_else(|error| {
            eprintln!("native-host-launcher: spawn failed: {error}");
            1
        });
    std::process::exit(status);
}
