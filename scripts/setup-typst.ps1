# Fgpui Typst sidecar 准备脚本
# 将仓库根目录的固定基线 typst.exe 复制为 Tauri externalBin 约定的
# 命名（binaries/typst-<target-triple>.exe），供 tauri dev / build 使用。
#
# 用法：pwsh -File scripts/setup-typst.ps1

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repoRoot "typst.exe"
$binDir = Join-Path $repoRoot "src-tauri\binaries"

if (-not (Test-Path $source)) {
    Write-Error "未找到基线 Typst: $source"
    exit 1
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null

# windows-gnu 为主构建目标（src-tauri/.cargo/config.toml 固定）
$targets = @("typst-x86_64-pc-windows-gnu", "typst-x86_64-pc-windows-msvc")
foreach ($t in $targets) {
    $dest = Join-Path $binDir "$t.exe"
    Copy-Item $source $dest -Force
    Write-Host "已复制 $source -> $dest"
}

$version = & $source --version
Write-Host "基线版本: $version"
Write-Host "完成。"
