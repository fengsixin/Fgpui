# Fgpui 应用图标生成脚本（阶段 0 占位图标）
# 使用 System.Drawing 生成 1024x1024 的应用图标源图，随后由
# `npx tauri icon` 生成全尺寸图标集合（icon.ico / 各尺寸 PNG）。

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$outPath = Join-Path $PSScriptRoot "..\app-icon.png"
$size = 1024

$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

# Element Plus 主色调背景 + 圆角
$bg = [System.Drawing.Color]::FromArgb(255, 64, 158, 255)
$g.Clear($bg)

$brush = [System.Drawing.Brushes]::White
$font = New-Object System.Drawing.Font("Segoe UI", 560, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$format = New-Object System.Drawing.StringFormat
$format.Alignment = [System.Drawing.StringAlignment]::Center
$format.LineAlignment = [System.Drawing.StringAlignment]::Center
$rect = New-Object System.Drawing.RectangleF(0, -30, $size, $size)
$g.DrawString("F", $font, $brush, $rect, $format)

$g.Dispose()
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

Write-Host "图标源已生成: $outPath"
