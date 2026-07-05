# Changelog

## v0.1.1 (2026-07-05)

### 新增

- **Windows 支持**：Win11 x64 构建（.exe / .msi 安装包）
- **跨平台字体**：Typst 模板自动适配系统字体（macOS PingFang SC / Windows 微软雅黑 / Linux Noto）
- **Typst sidecar 自动下载**：构建时按平台下载，不再提交大文件到仓库
- GitHub Actions 同时构建 3 个 target（macOS arm64 / macOS x86_64 / Windows x64）

### 变更

- `.gitignore` 忽略 typst 二进制
- printing.rs 字体路径改用 `cfg!(target_os)` 跨平台判断
- 新增 `scripts/download-typst.sh` 本地开发下载脚本

---

## v0.1.0 (2026-07-05)

首个发布版本。

### 核心功能

- **任务管理**：新建、编辑、删除任务，弹出式编辑器
- **多种频率**：每天 N 次 / 每周指定日 / 每 N 天 / 单次 / 日历手动选日期
- **时间段绑定**：每个任务绑定一个或多个时间段（硬绑定），支持无固定时间的"随时"任务
- **优先级**：紧急 / 重要 / 一般 / 可选（4 档）
- **当日打卡表**：按日期生成，按时段排序，无时段任务排末尾
- **冲突检测**：两个任务时段重叠时告警
- **待定标记**：当日视图临时标记任务为"待定"，原位变灰划线 + 末尾追加副本，PDF 只显示末尾
- **打印**：Typst 生成 A4 PDF（中文排版、表格、复选框、复盘区），系统查看器打开
- **任务要求**：每个任务可填执行标准/注意事项，显示在卡片和 PDF 备注列

### 技术栈

- **外壳**：Tauri 2
- **前端**：Leptos 0.8（CSR，WASM）
- **后端**：Rust（tauri commands）
- **存储**：rusqlite（bundled SQLite）+ refinery 迁移
- **打印**：Typst CLI（sidecar）

### 已知限制

- 仅支持 macOS（Apple Silicon + Intel）
- 未签名（需右键打开）
- 打印依赖系统字体（PingFang SC）
