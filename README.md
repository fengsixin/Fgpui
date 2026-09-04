# Fgpui 单机标准文档生成工具

Windows 优先的单机桌面应用：选择文档模板 → 填写表单或导入数据 → 生成标准 PDF → 预览和导出。

开发计划见 `1.md`。本 README 记录工程结构、开发命令与当前阶段状态。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2（windows-gnu 工具链） |
| 前端 | Vue 3 + TypeScript + Vite |
| UI | Element Plus |
| 状态管理 | Pinia |
| 核心逻辑 | Rust |
| 排版引擎 | Typst 0.15.1（`C:\code\Fgpui\typst.exe` 固定基线，以 sidecar 随包分发） |

## 目录结构

```text
C:\code\Fgpui\
├── 1.md                  # 开发计划（原始需求）
├── typst.exe             # Typst 0.15.1 固定基线（本地文件，不入版本库；由 setup-typst.ps1 接入 sidecar）
├── package.json          # 前端依赖与脚本
├── vite.config.ts
├── src\                  # Vue 3 前端
│   ├── api\              # Tauri invoke 封装与类型
│   ├── stores\           # Pinia
│   ├── views\            # 页面
│   └── router\
├── scripts\              # 辅助脚本（setup-typst / make-icon）
└── src-tauri\            # Rust 后端
    ├── src\
    │   ├── lib.rs        # 应用入口
    │   ├── error.rs      # 统一错误结构（AppError）
    │   ├── logging.rs    # 统一日志（滚动文件 + stdout）
    │   ├── paths.rs      # 工作区目录（Documents\FgpuiDocuments）
    │   ├── typst.rs      # Typst CLI 封装 + 诊断解析
    │   └── commands\     # Tauri commands
    ├── resources\hello.typ   # 阶段 0 冒烟测试模板
    ├── binaries\             # sidecar 二进制（git 忽略，由 setup 脚本生成）
    └── .cargo\config.toml    # 固定 x86_64-pc-windows-gnu 构建目标
```

## 用户数据目录

```text
%USERPROFILE%\Documents\FgpuiDocuments\
├── projects\   # 项目（每项目一个子目录）
├── templates\  # 文档模板包
├── fonts\      # 应用分发的字体
├── backups\    # 备份
└── logs\       # 运行日志（fgpui.log，按天滚动）
```

## 开发命令

```powershell
npm install --ignore-scripts --cache .npm-cache   # 安装前端依赖（沙箱环境跳过 postinstall）
pwsh -File scripts\setup-typst.ps1                # 复制 Typst 基线为 sidecar 命名
npm run tauri dev                                 # 启动开发版应用（Vite + Tauri）
npm run build                                     # 前端类型检查 + 构建
cargo test                                        # 在 src-tauri 内运行 Rust 测试
```

## 环境注意（本机）

- 未安装 Visual Studio Build Tools，MSVC 目标无法链接；工程固定使用 `x86_64-pc-windows-gnu`
  （`src-tauri/.cargo/config.toml`），已验证 mingw64（`C:\mingw64\bin`）+ WebView2 运行时可用。
- sidecar 命名相应为 `typst-x86_64-pc-windows-gnu.exe`。

## 阶段状态

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| 0 | 工程初始化与 Typst 可行性验证 | 进行中 |
| 1 | 本地项目与工作区管理 | 未开始 |
| 2 | 模板包和数据模型 | 未开始 |
| 3 | Typst 编译与 PDF 预览 | 未开始 |
| 4 | Excel/JSON 导入与文档内容完善 | 未开始 |
| 5 | 模板版本、可复现生成与质量保障 | 未开始 |
| 6 | Windows 发布与稳定性验收 | 未开始 |
