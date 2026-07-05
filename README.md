# 每日计划表 · DailyPlan

一个本地桌面应用：录入带**频率 + 时长**的任务，绑定时间段，按日生成**打卡表**并打印（纸质检查用）。

## 功能

- **任务管理**：新建 / 编辑 / 删除任务，每个任务绑定一个或多个时间段（硬绑定）
- **多种频率**：每天 N 次 / 每周指定日 / 每 N 天 / 单次
- **当日打卡表**：选日期 → 自动展开当天应做的任务，按时段排序
- **冲突检测**：两个任务时段重叠时给出告警（首尾相接不算冲突）
- **打印**：一键生成 A4 排版的 PDF（Typst 渲染），用系统查看器打开，Cmd+P 打印

## 技术栈

| 层 | 技术 |
|---|---|
| 外壳 | Tauri 2 |
| 前端 | Leptos 0.8（CSR，编译到 WASM，Trunk 打包） |
| 后端逻辑 | Rust（通过 Tauri command 暴露） |
| 存储 | rusqlite（bundled SQLite）+ refinery 迁移 |
| 打印 | Typst CLI（作为 Tauri sidecar） |

### 为什么 Typst 而不是浏览器打印？

macOS 的 WKWebView 不响应 `window.print()`，所以「HTML + Ctrl+P」走不通。
本应用改为：Leptos 在屏幕上渲染可预览的 HTML 表格；点「打印」时由 Rust 调
Typst sidecar 生成 PDF，再用系统查看器打开。Typst 模板内置：
- A4 排版、CJK 字体（PingFang SC 优先）
- 表格：时间 / 任务 / 时长 / 完成（空复选框）/ 备注
- 底部「今日复盘 / 明日改进」手写区
- 冲突告警区

## 项目结构

```
DailyPlan/                      Cargo workspace
├── Cargo.toml                  前端 crate (dailyplan-ui) + workspace 根
├── crates/
│   ├── domain/                 共享类型：Task / Frequency / TimeSlot / DayPlan
│   └── engine/                 调度 + 冲突检测 + Typst 渲染数据准备
│       └── templates/checklist.typ
├── src-tauri/                  Tauri 桌面应用
│   ├── binaries/typst-aarch64-apple-darwin   Typst sidecar 二进制
│   ├── capabilities/           shell 执行 sidecar 的权限
│   ├── migrations/             SQLite schema
│   └── src/                    db / commands / printing
├── src/                        前端 Leptos 组件 (app / day_view / task_editor / task_list)
└── styles.css
```

## 开发环境准备

需要 macOS（Apple Silicon）。首次安装工具链：

```bash
# 1. 安装 Rust（国内用清华镜像加速）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# 2. 配置 cargo 清华镜像（可选但强烈建议）
mkdir -p ~/.cargo && cat > ~/.cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "tuna"
[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
[net]
git-fetch-with-cli = true
EOF

# 3. 装 wasm target 和工具
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli --locked
cargo install wasm-bindgen-cli --version 0.2.126 --locked

# 4. Typst sidecar 二进制（已放在 src-tauri/binaries/，无需再下）
```

## 运行

```bash
cargo tauri dev
```

## 打包发布

```bash
cargo tauri build
# 产物在 src-tauri/target/release/bundle/
```

## 数据存储位置

数据库与生成的 PDF 在：
- macOS: `~/Library/Application Support/com.dailyplan.app/`
  - `dailyplan.db` — 任务库
  - `print/` — Typst 模板、data.json、生成的 PDF

## 测试

```bash
cargo test                    # 全部
cargo test -p dailyplan-domain       # 频率匹配 / 时长
cargo test -p dailyplan-engine       # 调度 / 冲突检测 / 渲染数据
cargo test -p dailyplan              # 数据库 CRUD + 频率往返
```

## 二期规划（暂未实现）

- 软时段自动调度（给时段池让程序自动排）
- 日期范围 / 月度频率（习惯冲刺）
- 屏幕端打卡状态持久化（check_records 表）
- Windows / Linux 的 Typst sidecar 与打印路径适配
