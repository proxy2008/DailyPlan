# DailyPlan 多页面 UI 重构 + 任务管理增强 + 多日打印

- 分支：`feat/multi-page-ui`（基于 `main` @ v0.1.1）
- 日期：2026-07-05
- 目标版本：v0.2.0

## 一、目标

把当前单页双栏布局重构为「左侧栏导航 + 双页面」结构，并完成三项增强：

1. **导航重构**：日程页 / 任务管理页 两个独立页面，左侧常驻 sidebar 切换
2. **任务管理增强**：模糊搜索 + 按频率/优先级筛选 + 排序 + 按优先级分组展示
3. **多日连续打印**：打印按钮改下拉菜单，支持「打印当天」「打印多日」；多日从今天起连续 N 天，合并到单个 PDF

## 二、整体架构

### 2.1 方案选型：信号驱动的页面切换

App 顶层持有一个 `Page` 信号（`Schedule` / `TaskManage`），左侧 sidebar 点击切换。
主区域 `match page.get()` 渲染对应组件。

不采用 `leptos_router` 的理由：桌面 Tauri 应用 URL 无意义，路由间共享状态要靠
context/global store，比信号直传绕；信号切换最贴合 Leptos 0.8 CSR 的响应式模型。

### 2.2 状态上提

当前 `app.rs` 已持有 `tasks`/`set_tasks`/`tasks_rev`/`date`/`refresh`/`editor_state`/
`confirming`。这些信号继续留在 App 顶层，两个页面通过 props 接收 `ReadSignal` 或
`RwSignal`：

| 信号          | 类型                           | 谁用                       |
|---------------|--------------------------------|----------------------------|
| `page`        | `RwSignal<Page>`              | Sidebar（写）/ App（读）   |
| `tasks`       | `ReadSignal<Vec<Task>>`        | Schedule + TaskManage      |
| `date`        | `RwSignal<String>`            | Schedule                   |
| `refresh`     | `StoredValue<Fn()>`           | App 内部（保存/删除后）    |
| `editor_state`| `RwSignal<EditorState>`       | App + TaskManage（编辑）   |
| `confirming`  | `RwSignal<Option<i64>>`       | TaskManage                 |
| `tasks_rev`   | `RwSignal<u32>`               | TaskManage（For 重建）     |

### 2.3 全局事件委托不变

`app.rs` 里 document 级 click 委托（处理 `data-action="edit/delete/confirm-delete/
cancel-delete"`）保持不动。任务管理页的卡片按钮继续用 `data-action` + `data-task-id`，
新增的 `data-action="edit-from-manage"` 也走同一通道（行为同 edit，只是回到日程页可省）。

## 三、应用骨架与导航

### 3.1 布局结构

```
┌─────────────────────────────────────────────────────────┐
│ Sidebar (60px) │          Main Area                      │
│                │                                        │
│   🗓 日程       │   (Schedule 或 TaskManage 组件)         │
│   📋 任务       │                                        │
│                │                                        │
│   ──────       │                                        │
│   (底部留白)    │                                        │
└─────────────────────────────────────────────────────────┘
```

- Sidebar 固定 60px 宽，纵向排列图标按钮；当前页高亮（`--primary` 背景或左侧色条）
- 主区域占满剩余空间，独立滚动
- 暗黑模式下 sidebar 用稍深的背景（新 CSS 变量 `--sidebar-bg`）
- 编辑器 modal 继续浮在最上层（z-index 1000 不变）

### 3.2 Page 枚举

```rust
#[derive(Clone, Copy, PartialEq)]
enum Page { Schedule, TaskManage }
```

### 3.3 新增文件

- `src/sidebar.rs`：`Sidebar(page: RwSignal<Page>)` 组件，纯渲染 + `on:click` 切 page

## 四、日程页（Schedule）

### 4.1 迁移自现有 DayView

新增 `src/schedule.rs` 作为日程页 wrapper：它持有日期工具条 + 打印下拉菜单 +
DayView（纯展示）。`day_view.rs` 内部去掉打印按钮，只负责按 `date` 加载并渲染
DayPlan + 待定区，改成全宽展示（原来是左栏 1.4fr）。

- `Schedule` wrapper：日期工具条 + 打印下拉 + `<DayView>`
- `DayView`：纯展示（计划表 + 待定区），不再含打印按钮，改成全宽
- 打印下拉在 Schedule wrapper 里（见 4.2）

### 4.2 打印下拉菜单

打印按钮变成一个带 ▾ 的容器，点击展开两个选项：

```
┌─────────────┐
│ 🖨 打印 ▾    │  ← 点击展开
└─────────────┘
   ┌──────────────────┐
   │ 🖨 打印当天        │  → 现有 print_day 逻辑
   │ 📅 打印多日…      │  → 弹多日对话框
   └──────────────────┘
```

实现：一个 `RwSignal<bool>` 的 `print_menu_open`，点击按钮 toggle；
菜单项用 `on:click`（不在 For 内，Leptos 0.8 的 on:click 稳定）。
点击外部关闭：在 overlay div 上 `on:click` 关闭（参考 modal 的 stop_propagation 模式）。

### 4.3 多日打印对话框

modal 弹窗（复用 `.modal-overlay` / `.modal-content` 样式）：

```
┌──────────────────────────────┐
│  打印多日                     │
│                              │
│  从今天（2026-07-05）起       │
│  连续 [ 7  ] 天              │
│                              │
│  将生成 7 天的打卡表，        │
│  每天一页，合并到 1 个 PDF。  │
│                              │
│        [取消]  [生成 PDF]    │
└──────────────────────────────┘
```

- 输入框：天数 N（默认 7，范围 1–31，校验）
- 点击「生成 PDF」→ 调后端 `print_days` 命令
- 关键决策（已与用户确认）：多日 PDF **不处理待定**，每天按原计划时段打印，
  无「待定」列、不重排。每天的 items 直接来自 `generate_day(date)` 的原始结果。

## 五、任务管理页（TaskManage）

### 5.1 工具条

```
┌─────────────────────────────────────────────────────────┐
│ [+ 新建任务]  [搜索框________] [频率▾][优先级▾] [排序▾]  │
└─────────────────────────────────────────────────────────┘
```

- **新建任务** 按钮：`start_create()` 逻辑不变
- **搜索框**：实时模糊匹配（on:input），匹配 name + description（要求字段）
- **频率筛选**：下拉「全部 / 每天 / 每周 / 单次 / 指定日期 / 间隔」，按 Frequency kind 匹配
- **优先级筛选**：下拉「全部 / 紧急 / 重要 / 一般 / 可选」
- **排序**：下拉「按优先级（默认） / 按名称 / 按创建时间」，可切换升降序

### 5.2 派生信号：filtered_tasks

用 `Memo` 派生，输入信号：`tasks` + `search_kw` + `freq_filter` + `pri_filter` +
`sort_key` + `sort_dir`。每次任意输入变化自动重算。

```rust
let filtered = Memo::new(move |_| {
    let kw = search_kw.get().to_lowercase();
    let freq = freq_filter.get();
    let pri = pri_filter.get();
    let mut v: Vec<Task> = tasks.get().into_iter()
        .filter(|t| kw.is_empty() || t.name.to_lowercase().contains(&kw)
                    || t.description.as_deref().unwrap_or("").to_lowercase().contains(&kw))
        .filter(|t| freq.is_none() || freq_kind(&t.frequency) == freq.unwrap())
        .filter(|t| pri.is_none() || t.priority_level == pri.unwrap())
        .collect();
    sort_tasks(&mut v, sort_key.get(), sort_dir.get());
    v
});
```

### 5.3 按优先级分组展示

`filtered` 派生后，再按 `priority_level` 分组成 4 个桶（顺序：紧急 → 重要 → 一般 → 可选）。
空桶不显示。每个分组有一个小标题（如「紧急（3）」），下方是该组的卡片列表。

如果排序键不是「按优先级」，则不分组，平铺展示（避免「按名称排序却分组」的矛盾）。

### 5.4 卡片复用

卡片渲染逻辑从 `task_list.rs` 抽到 `task_list.rs::render_card(t, confirming)` 复用。
TaskManage 页用 filtered + 分组渲染；删除/编辑事件继续走 document 级委托。

### 5.5 空态

- 任务总数 0：「还没有任务，点击「新建任务」开始添加。」（现有文案）
- 有任务但筛选后为 0：「没有匹配的任务，试试调整搜索或筛选条件。」

## 六、多日打印后端

### 6.1 新命令：`print_days`

`src-tauri/src/commands.rs` 新增：

```rust
#[tauri::command]
pub async fn print_days(
    app: AppHandle,
    start_date: String,   // YYYY-MM-DD，camelCase: startDate
    days: u32,
) -> Result<String, AppError> {
    // 1. 从 start_date 起连续 days 天，每天调 scheduler 生成 DayPlan
    // 2. 收集成 Vec<PrintData>
    // 3. 写 data.json（数组）+ 用多日模板编译
    // 4. 打开 PDF，返回路径
}
```

**注意 Tauri 2 camelCase**：前端调用时参数名是 `startDate` / `days`。
Rust 端 snake_case，序列化层自动转换（这是项目里反复踩过的坑）。

### 6.2 后端直接生成每日 items（不经前端）

多日打印时，前端不传 items（无法逐日勾选待定），后端直接：
- 读 DB → 取所有 active tasks
- 对每个日期 `date`，用 `engine::scheduler::build_day_plan(tasks, date)` 生成 DayPlan
- 每个 DayPlan → `to_print_data(plan, opts)` 得到 PrintData
- 不传 pending 标记（默认 false），不重排

### 6.3 新 Typst 模板：`checklist_multi.typ`

基于现有 `checklist.typ` 改造，接受 `data.json` 为 **数组**：

```typst
#let days = json("data.json")   // array of PrintData

#for (i, day) in days.enumerate() [
  // ... 现有的标题区 + 表格 ...
  #if i < days.len() - 1 [
    #pagebreak()
  ]
]
```

- 每天一页（A4），用 `#pagebreak()` 分页
- **不渲染待定相关**：不显示「待定」列、不灰底、不重排（所有 item pending=false）
- 复盘区每天保留（每页底部都有「今日复盘 / 明日改进」）
- 冲突告警每天保留（如有）

### 6.4 渲染数据结构

`engine/src/render.rs` 新增：

```rust
pub fn to_print_data_multi(plans: &[DayPlan], opts: &RenderOptions) -> Vec<PrintData>
```

把多日数据序列化成 JSON 数组（`[PrintData, ...]`），现有单日 `PrintData` 结构复用。
为简化模板，多日模式下 `PrintItem.pending` 全部为 false，`note` 来自 task.description。

### 6.5 `printing.rs` 扩展

新增 `print_days(app, start_date, days)`：
- 循环构造 `Vec<PrintData>`
- 写 `data.json`（数组）
- 写 `checklist_multi.typ`
- spawn typst 编译，输出 `dailyplan-{start}-to-{end}.pdf`
- opener 打开

模板常量：`engine/src/render.rs` 新增 `pub const CHECKLIST_MULTI_TYP: &str = include_str!(...)`。

## 七、前端 invoke 封装

`src/tauri.rs` 新增：

```rust
pub async fn print_days(start_date: &str, days: u32) -> Result<String, String>
```

参数序列化注意：`#[serde(rename = "startDate")]` 在 wrapper 结构上，或用 `serde_json`
手工构造 args 对象（项目现有模式是手工构造，沿用）。

## 八、CSS 改动

新增样式（styles.css）：

- `.app-shell`：flex 容器，sidebar + main
- `.sidebar`：60px 固定宽，纵向 flex，背景 `--sidebar-bg`
- `.sidebar-btn`：图标按钮，48×48，当前页高亮
- `.nav-current`：高亮样式（左侧色条或背景色）
- `.page`：主区域容器，padding + overflow-y: auto
- `.print-dropdown`：打印下拉容器
- `.print-menu`：下拉菜单项
- `.multi-day-dialog`：多日弹窗（复用 modal 样式，加一个 number input）
- `.toolbar-row`：任务管理工具条
- `.search-input` / `.filter-select`：搜索框 + 下拉筛
- `.group-header`：优先级分组标题
- 暗黑模式：新增 `--sidebar-bg`、`--sidebar-active` 变量及对应 dark 覆盖

## 九、文件改动清单

### 新增

| 文件 | 内容 |
|------|------|
| `src/sidebar.rs` | Sidebar 组件 |
| `src/schedule.rs` | 日程页 wrapper（工具条 + 打印下拉 + DayView） |
| `src/task_manage.rs` | TaskManage 页（工具条 + filtered Memo + 分组渲染） |
| `crates/engine/templates/checklist_multi.typ` | 多日 Typst 模板 |

### 修改

| 文件 | 改动 |
|------|------|
| `src/app.rs` | 加 `Page` 枚举 + `page` 信号；主区域改 `match page.get()`；sidebar + page 布局；打印下拉菜单 + 多日弹窗状态 |
| `src/day_view.rs` | 去掉内嵌打印按钮和日期工具条（移到 Schedule wrapper）；只保留计划表 + 待定区渲染，改成全宽 |
| `src/schedule.rs`（新） | 日程页 wrapper：日期工具条 + 打印下拉菜单 + DayView |
| `src/task_list.rs` | 抽 `render_card(t, confirming)` 为独立 pub 函数供复用；TaskList 组件本身被 TaskManage 取代后删除 |
| `src/tauri.rs` | 加 `print_days` 包装 |
| `src/main.rs` | 可能要 mod 声明新模块 |
| `src-tauri/src/commands.rs` | 加 `print_days` 命令 |
| `src-tauri/src/lib.rs` | 注册 `print_days` 到 invoke handler |
| `src-tauri/src/printing.rs` | 加 `print_days` 函数 |
| `crates/engine/src/render.rs` | 加 `CHECKLIST_MULTI_TYP` + `to_print_data_multi` |
| `src-tauri/tauri.conf.json` | version → 0.2.0；窗口默认尺寸可能加宽到 1200×780 |
| `Cargo.toml` | version → 0.2.0 |
| `styles.css` | 新增 sidebar / page / dropdown / toolbar 样式 + dark 模式 |

### 删除

- `src/task_list.rs`：被 TaskManage 取代后整体删除（render_card 抽到 `task_manage.rs`）

## 十、实施顺序

1. **骨架**：`sidebar.rs` + `app.rs` 改 Page 切换 + CSS shell 布局。先跑通「点 sidebar 切换空白页」
2. **日程页迁移**：把 DayView 放进 Schedule 页，确认功能不退化
3. **任务管理页基础**：TaskManage 页用现有 TaskList 渲染（无筛选），确认编辑/删除/新建通过 document 委托仍工作
4. **任务管理增强**：搜索 + 频率/优先级筛选 + 排序 + 分组
5. **打印下拉菜单**：单日打印按钮改下拉，先只放「打印当天」
6. **多日打印后端**：`print_days` 命令 + `checklist_multi.typ` + `to_print_data_multi`
7. **多日打印前端**：弹窗 + 调用 `print_days`
8. **样式打磨 + 暗黑模式**：sidebar、下拉、弹窗、工具条的 dark 模式
9. **版本号 → 0.2.0，更新 CHANGELOG**

每步完成后 `cargo check` + `trunk build` 验证编译，关键交互用 Playwright 截图验证。

## 十一、不做的事（YAGNI）

- ❌ URL 路由（leptos_router）—— 桌面应用不需要
- ❌ 多日 PDF 的待定列 —— 用户明确选「不处理待定」
- ❌ 多日 PDF 的日期范围选择器 —— 用户明确选「从今天起 N 天」
- ❌ 任务标签/分类 —— 当前 4 档优先级 + 频率筛选已够用
- ❌ 拖拽排序 —— 排序靠下拉切换，不做拖拽
- ❌ 任务归档/软删除 —— 范围外
- ❌ 国际化 —— 继续中文优先

## 十二、风险与对策

| 风险 | 对策 |
|------|------|
| 状态上提后信号所有权混乱 | App 持有所有 RwSignal，子组件只收 ReadSignal 或克隆的 RwSignal（Leptos 信号是 Copy 的） |
| document 级事件委托在新布局下失效 | 不变 handler 注册位置（仍在 App 顶层 spawn_local），TaskManage 的卡片同样用 data-action |
| 多日 Typst 模板分页错乱 | 先用 2 天数据测，确认 pagebreak 位置；复盘区高度固定避免溢出 |
| Tauri 2 camelCase 坑再现 | print_days 的 `start_date` → 前端必须传 `startDate`；写完命令立即用 debug 面板验证 |
| filtered Memo 触发 For 频繁重建 | For key 用 `(tasks_rev.get(), t.id)` 不变；filtered 只是改变 each 的输入 |
| 窗口宽度变化导致布局塌 | sidebar 固定 60px，main 用 flex:1；保留 @media 响应式断点 |
