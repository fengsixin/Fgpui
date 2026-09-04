use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    embed_windows_manifest();
    tauri_build::build()
}

/// 将 Common-Controls v6 清单嵌入**所有**链接目标（bin 与 test）。
///
/// 背景：windows-gnu 下 tao 依赖 comctl32 v6 的 `TaskDialogIndirect` 等入口；
/// tauri-build 只给 bin 目标嵌入清单（link-arg-bins），裸 `cargo test` 的测试
/// 可执行文件没有清单，加载 comctl32 v5 时报 STATUS_ENTRYPOINT_NOT_FOUND。
/// 这里用 windres 把清单编译成 COFF 目标并附加到所有链接命令上；
/// 资源 ID 取 2，避免与 tauri-build 给 bin 嵌入的 ID 1 清单冲突。
fn embed_windows_manifest() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows");
    let rc = manifest_dir.join("fgpui-manifest.rc");
    let obj = PathBuf::from(env::var("OUT_DIR").unwrap()).join("fgpui-manifest.o");

    let windres = env::var("WINDRES").unwrap_or_else(|_| "windres".to_string());
    let status = Command::new(&windres)
        .arg("-i")
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .current_dir(&manifest_dir)
        .status()
        .unwrap_or_else(|e| panic!("无法运行 windres（{windres}）: {e}"));
    if !status.success() {
        panic!("windres 编译清单失败: {}", rc.display());
    }

    println!("cargo:rustc-link-arg={}", obj.display());
    println!("cargo:rerun-if-changed=windows/fgpui-manifest.rc");
    println!("cargo:rerun-if-changed=windows/fgpui-manifest.xml");
}
