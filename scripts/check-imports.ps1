# 诊断脚本：核对 PE 导入表中每个函数在目标 DLL 导出表中的存在性
# 用法: pwsh -File scripts\check-imports.ps1 <exe路径>
param([Parameter(Mandatory = $true)][string]$ExePath)

$ErrorActionPreference = "Stop"
$objdump = "C:\mingw64\bin\objdump.exe"

function Get-DllExports([string]$dllPath) {
    $text = & $objdump -p $dllPath 2>$null | Out-String
    $names = [System.Collections.Generic.HashSet[string]]::new()
    # 兼容两种格式:
    #   [   0] +base[   1]  0000 CompareBrowserVersions   (MSVC PE32+)
    #   [    27] DefSubclassProc                          (部分工具链)
    foreach ($line in ($text -split "`r?`n")) {
        if ($line -match '^\s*\[\s*\d+\]') {
            $tokens = $line.Trim() -split '\s+'
            $last = $tokens[$tokens.Count - 1]
            if ($last -match '^[A-Za-z_][A-Za-z0-9_]*$') {
                [void]$names.Add($last)
            }
        }
    }
    return $names
}

# 1) 解析 exe 导入表: DLL -> 函数列表
$dump = & $objdump -p $ExePath | Out-String
$imports = [ordered]@{}
$current = $null
foreach ($line in ($dump -split "`r?`n")) {
    if ($line -match '^\s*DLL Name:\s*(\S+)') {
        $current = $Matches[1]
        $imports[$current] = [System.Collections.Generic.List[string]]::new()
    }
    elseif ($current -and $line -match '^\s*[0-9a-f]{6,}\s+<none>\s+\S+\s+(\S+)') {
        $imports[$current].Add($Matches[1])
    }
}

$exeDir = Split-Path -Parent $ExePath
$sys32 = Join-Path $env:WINDIR "System32"
$pathDirs = $env:PATH -split ';' | Where-Object { $_ }

$totalMissing = 0
foreach ($dll in $imports.Keys) {
    $funcs = $imports[$dll]
    if ($funcs.Count -eq 0) { continue }

    # 按加载器搜索顺序解析 DLL
    $resolved = $null
    foreach ($cand in @((Join-Path $exeDir $dll), (Join-Path $sys32 $dll)) + ($pathDirs | ForEach-Object { Join-Path $_ $dll })) {
        if ($cand -and (Test-Path $cand)) { $resolved = $cand; break }
    }
    if (-not $resolved) {
        Write-Host ("[未找到] {0}  (导入 {1} 个函数)" -f $dll, $funcs.Count) -ForegroundColor Red
        $totalMissing += $funcs.Count
        continue
    }

    $exports = Get-DllExports $resolved
    if ($exports.Count -eq 0) {
        Write-Host ("[无法核对] {0} -> {1} (导出表解析为空, 跳过)" -f $dll, $resolved) -ForegroundColor Yellow
        continue
    }
    $missing = $funcs | Where-Object { -not $exports.Contains($_) }
    if ($missing) {
        Write-Host ("[缺入口] {0} -> {1}" -f $dll, $resolved) -ForegroundColor Red
        foreach ($f in $missing) { Write-Host ("    missing: {0}" -f $f) }
        $totalMissing += $missing.Count
    }
    else {
        Write-Host ("[OK] {0} -> {1} ({2} 函数全部命中)" -f $dll, $resolved, $funcs.Count) -ForegroundColor Green
    }
}
Write-Host ""
Write-Host ("缺失入口点总数: {0}" -f $totalMissing)
if ($totalMissing -gt 0) { exit 1 } else { exit 0 }
