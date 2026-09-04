//! 集成测试：调用真实 Typst 基线二进制编译 hello.typ。
//! 通过环境变量 FGPU_TYPST_EXE 可指定路径；默认解析仓库根目录 typst.exe。

use std::path::PathBuf;
use std::process::Command;

fn baseline_typst() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FGPU_TYPST_EXE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("..").join("..").join("typst.exe");
    if root.is_file() {
        return Some(root);
    }
    None
}

#[test]
fn baseline_typst_reports_version() {
    let Some(exe) = baseline_typst() else {
        eprintln!("跳过：未找到基线 typst.exe");
        return;
    };
    let out = Command::new(&exe).arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("typst"), "unexpected version output: {stdout}");
}

#[test]
fn compiles_hello_typ_to_pdf() {
    let Some(exe) = baseline_typst() else {
        eprintln!("跳过：未找到基线 typst.exe");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("hello.typ");
    let output = dir.path().join("hello.pdf");
    std::fs::write(&input, include_str!("../resources/hello.typ")).unwrap();

    let status = Command::new(&exe)
        .arg("compile")
        .arg("--root")
        .arg(dir.path())
        .arg(&input)
        .arg(&output)
        .status()
        .unwrap();

    assert!(status.success(), "typst compile failed");
    assert!(output.is_file(), "PDF was not produced");
    let size = std::fs::metadata(&output).unwrap().len();
    assert!(size > 500, "PDF suspiciously small: {size} bytes");

    let head = std::fs::read(&output).unwrap();
    assert!(head.starts_with(b"%PDF"), "output is not a PDF");
}

#[test]
fn compile_failure_returns_nonzero_and_parseable_diagnostics() {
    let Some(exe) = baseline_typst() else {
        eprintln!("跳过：未找到基线 typst.exe");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bad.typ");
    let output = dir.path().join("bad.pdf");
    std::fs::write(&input, "#unknown-variable-that-does-not-exist").unwrap();

    let out = Command::new(&exe)
        .arg("compile")
        .arg("--root")
        .arg(dir.path())
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();

    assert!(!out.status.success(), "expected failure exit code");
    assert!(!output.exists(), "failed compile must not produce PDF");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let diags = fgpui_lib::typst::parse_diagnostics(&stderr);
    assert!(
        diags.iter().any(|d| d.severity == "error"),
        "no error diagnostic parsed from: {stderr}"
    );
}
