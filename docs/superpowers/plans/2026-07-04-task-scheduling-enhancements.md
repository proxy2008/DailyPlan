# 任务调度增强 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 DailyPlan 增加四项能力：日历选日期频率、无时段任务、优先级文字级别（4档）、当日"待定"临时标记。

**Architecture:** 改动从 domain 层（共享类型）向外辐射到 engine（调度/渲染）、src-tauri（DB/命令）、frontend（编辑器/视图/打印）。按"垂直切片 + 测试先行"组织：先改 domain 类型并配单测，再让 engine/DB 适配，再让前端 UI 适配，最后接入"待定"的端到端流程。ChecklistItem 字段变更（Option 时间 + priority + pending）是核心传导点，必须最先做。

**Tech Stack:** Rust + Tauri 2 + Leptos 0.8 CSR + rusqlite + Typst。

## Global Constraints

- Rust edition 2021，stable toolchain（rustc 1.96+）
- 所有 domain 类型派生 `Serialize, Deserialize`，serde tag 用 `rename_all = "snake_case"`
- DB 复用现有 `priority INTEGER` 列存 `PriorityLevel::rank()`（0-3），不新增列
- 旧数据用户已同意清空重建——不需要写数据映射迁移，但 `priority` 列默认值要保证新任务为 Normal(1)
- 所有改动遵循现有模式：domain 无 IO、engine 纯逻辑、commands 薄包装、printing 调 sidecar
- 测试命令：`cargo test --workspace`（全部）；`cargo check -p dailyplan-ui --target wasm32-unknown-unknown`（前端）
- 前端 rebuild：`trunk build`（dev 模式，避免 wasm-opt 下载）
- 项目根：`/Users/tengyouting/code/python/DailyPlan`

---

## File Structure

改动文件分层：

- `crates/domain/src/task.rs` — 加 `PriorityLevel` 枚举；`Frequency::Custom`；`Task.priority` → `priority_level`；`weekday_to_index` 改 `pub(crate)`
- `crates/domain/src/checklist.rs` — `ChecklistItem` 加 Option 时间 + priority + pending 字段
- `crates/engine/src/scheduler.rs` — 空 slots 分支；新排序键
- `crates/engine/src/conflict.rs` — 跳过无时段 item
- `crates/engine/src/render.rs` — `PrintItem` 加 `time: Option<String>` + `pending: bool`；`to_print_data` 适配
- `crates/engine/templates/checklist.typ` — pending 行灰色样式
- `src-tauri/src/db.rs` — `priority i32 ↔ PriorityLevel` 转换；Custom 往返测试
- `src-tauri/src/commands.rs` — `print_day` 签名改（接 items）
- `src-tauri/src/printing.rs` — 用传入 items 渲染，不再查 DB/调度
- `src/task_editor.rs` — untimed 复选框；优先级 select；日历组件
- `src/task_list.rs` — 优先级徽章；freq_label Custom 分支
- `src/day_view.rs` — pending 信号 + 双显示；print 传 items
- `src/tauri.rs` — print_day 改传 items
- `src/app.rs` — on_print 接 items
- `styles.css` — .pending 样式 + 优先级徽章颜色

---

## Task 1: PriorityLevel 枚举 + Frequency::Custom + Task 字段改造（domain 层）

**Files:**
- Modify: `crates/domain/src/task.rs`
- Modify: `crates/domain/src/lib.rs`（导出新类型）

**Interfaces:**
- Produces: `PriorityLevel { Urgent, High, Normal, Low }` with `rank()/from_rank()/label_cn()`
- Produces: `Frequency::Custom { dates: Vec<NaiveDate> }`
- Produces: `Task.priority_level: PriorityLevel`（替换原 `priority: i32`）
- Produces: `pub(crate) fn weekday_to_index(d: Weekday) -> usize`

- [ ] **Step 1: 加 PriorityLevel 枚举与 Frequency::Custom，并改 Task 字段**

修改 `crates/domain/src/task.rs`：

在 `Frequency` 枚举里加 `Custom` 变体（在 `Once` 之后）：
```rust
pub enum Frequency {
    Daily { times_per_day: u32 },
    Weekly { weekdays: [bool; 7] },
    Interval { every_days: u32, start: NaiveDate },
    Once { date: NaiveDate },
    /// 用户手动指定的若干日期（保持升序+去重）。
    Custom { dates: Vec<NaiveDate> },
}
```

在 `Frequency::matches` 加 `Custom` 分支（用 binary_search，因为构造时排序）：
```rust
Frequency::Custom { dates } => dates.binary_search(&date).is_ok(),
```

把 `weekday_to_index` 从私有改为 `pub(crate)`：
```rust
pub(crate) fn weekday_to_index(d: Weekday) -> usize {
```

新增 `PriorityLevel` 枚举（放在 `Frequency` 定义之前）：
```rust
/// 任务优先级（4 档）。用于排序与显示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLevel {
    Urgent,
    High,
    Normal,
    Low,
}

impl Default for PriorityLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl PriorityLevel {
    /// 数值越大越优先（用于排序）。
    pub fn rank(&self) -> i32 {
        match self {
            Self::Urgent => 3,
            Self::High => 2,
            Self::Normal => 1,
            Self::Low => 0,
        }
    }

    /// 整数 rank 反解（越界值 clamp）。
    pub fn from_rank(r: i32) -> Self {
        match r {
            r if r >= 3 => Self::Urgent,
            2 => Self::High,
            1 => Self::Normal,
            _ => Self::Low,
        }
    }

    pub fn label_cn(&self) -> &'static str {
        match self {
            Self::Urgent => "紧急",
            Self::High => "重要",
            Self::Normal => "一般",
            Self::Low => "可选",
        }
    }
}
```

把 `Task` 的 `priority: i32` 改为 `priority_level: PriorityLevel`：
```rust
pub struct Task {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub frequency: Frequency,
    #[serde(default)]
    pub slots: Vec<TimeSlot>,
    /// 冲突时谁让位；级别越高越优先。
    #[serde(default)]
    pub priority_level: PriorityLevel,
    #[serde(default = "default_active")]
    pub active: bool,
}
```

`Task::default` 同步改：
```rust
impl Default for Task {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            description: None,
            frequency: Frequency::default(),
            slots: Vec::new(),
            priority_level: PriorityLevel::default(),
            active: true,
        }
    }
}
```

更新 `lib.rs` 导出 `PriorityLevel`：
```rust
pub use checklist::{ChecklistItem, Conflict, DayPlan};
pub use task::{Frequency, PriorityLevel, Task, TimeSlot};
```

- [ ] **Step 2: 更新 task.rs 内的现有测试，加新测试**

更新 task.rs 的 `Frequency` 测试，加 `Custom` 用例：
```rust
#[test]
fn custom_matches_chosen_dates() {
    let f = Frequency::Custom {
        dates: vec![
            NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 8).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 12).unwrap(),
        ],
    };
    assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()));
    assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 12).unwrap()));
    assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 6).unwrap()));
}

#[test]
fn custom_empty_matches_nothing() {
    let f = Frequency::Custom { dates: vec![] };
    assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()));
}

#[test]
fn priority_level_rank_roundtrip() {
    for orig in [PriorityLevel::Urgent, PriorityLevel::High, PriorityLevel::Normal, PriorityLevel::Low] {
        let back = PriorityLevel::from_rank(orig.rank());
        assert_eq!(orig, back, "{:?} 往返不一致", orig);
    }
}

#[test]
fn priority_level_from_rank_clamps() {
    assert_eq!(PriorityLevel::from_rank(99), PriorityLevel::Urgent);
    assert_eq!(PriorityLevel::from_rank(-5), PriorityLevel::Low);
}
```

- [ ] **Step 3: 运行 domain 测试**

Run: `cargo test -p dailyplan-domain`
Expected: 全部通过（原有 5 个 + 新增 4 个 = 9 个）

- [ ] **Step 4: Commit**

```bash
git add crates/domain/src/task.rs crates/domain/src/lib.rs
git commit -m "feat(domain): add PriorityLevel + Frequency::Custom, change Task.priority to priority_level"
```

---

## Task 2: ChecklistItem 字段改造（domain 层）

**Files:**
- Modify: `crates/domain/src/checklist.rs`

**Interfaces:**
- Produces: `ChecklistItem { task_id, task_name, start: Option<NaiveTime>, end: Option<NaiveTime>, duration_min, priority: PriorityLevel, pending: bool }`
- Consumes: `PriorityLevel` from Task 1

- [ ] **Step 1: 改 ChecklistItem 字段**

修改 `crates/domain/src/checklist.rs` 的 `ChecklistItem`：
```rust
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::task::PriorityLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub task_id: i64,
    pub task_name: String,
    /// None 表示无时段任务。
    #[serde(default)]
    pub start: Option<NaiveTime>,
    #[serde(default)]
    pub end: Option<NaiveTime>,
    pub duration_min: u32,
    #[serde(default)]
    pub priority: PriorityLevel,
    /// 当日临时"待定"标记。后端始终 false，由前端改。
    #[serde(default)]
    pub pending: bool,
}
```

`DayPlan::empty` 不变（items 为空）。

- [ ] **Step 2: 运行 domain 测试（确认编译过）**

Run: `cargo test -p dailyplan-domain`
Expected: 编译通过，测试全过

- [ ] **Step 3: Commit**

```bash
git add crates/domain/src/checklist.rs
git commit -m "feat(domain): ChecklistItem with Option<time>, priority, pending fields"
```

---

## Task 3: 调度器 + 冲突检测适配（engine 层）

**Files:**
- Modify: `crates/engine/src/scheduler.rs`
- Modify: `crates/engine/src/conflict.rs`

**Interfaces:**
- Consumes: `ChecklistItem` 新字段（Task 2）、`PriorityLevel`（Task 1）
- Produces: `build_day_plan` 处理空 slots + 新排序键；`detect_conflicts` 跳过无时段 item

- [ ] **Step 1: 改 scheduler.rs 的 flat_map + 排序**

修改 `crates/engine/src/scheduler.rs` 的 `build_day_plan`，把 `flat_map` 改为先判断 slots 是否为空：

```rust
pub fn build_day_plan(date: NaiveDate, tasks: &[Task]) -> DayPlan {
    let mut items: Vec<ChecklistItem> = tasks
        .iter()
        .filter(|t| t.active && t.frequency.matches(date))
        .flat_map(|t| {
            if t.slots.is_empty() {
                // 无时段任务：产出单个 untimed item
                vec![ChecklistItem {
                    task_id: t.id,
                    task_name: t.name.clone(),
                    start: None,
                    end: None,
                    duration_min: 0,
                    priority: t.priority_level,
                    pending: false,
                }]
            } else {
                t.slots
                    .iter()
                    .map(move |slot| ChecklistItem {
                        task_id: t.id,
                        task_name: t.name.clone(),
                        start: Some(slot.start),
                        end: Some(slot.end),
                        duration_min: slot.duration_minutes(),
                        priority: t.priority_level,
                        pending: false,
                    })
                    .collect::<Vec<_>>()
            }
        })
        .collect();

    // 排序键：无时段 (start=None) 排最后；定时按 start 升序；
    // 同 start 按优先级降序；最后 task_id 升序稳定 tiebreak。
    items.sort_by(|a, b| {
        a.start.is_none().cmp(&b.start.is_none())
            .then_with(|| a.start.cmp(&b.start))
            .then_with(|| b.priority.rank().cmp(&a.priority.rank()))
            .then_with(|| a.task_id.cmp(&b.task_id))
    });

    let conflicts = detect_conflicts(&items);

    DayPlan {
        date,
        items,
        conflicts,
    }
}
```

- [ ] **Step 2: 改 conflict.rs 跳过无时段 item**

修改 `crates/engine/src/conflict.rs` 的 `detect_conflicts`，在内层循环开头加跳过逻辑：

```rust
pub fn detect_conflicts(items: &[ChecklistItem]) -> Vec<Conflict> {
    let mut out = Vec::new();
    let n = items.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let a = &items[i];
            let b = &items[j];
            // 无时段 item 不参与冲突检测
            if a.start.is_none() || b.start.is_none() {
                continue;
            }
            let (a_start, a_end) = (a.start.unwrap(), a.end.unwrap());
            let (b_start, b_end) = (b.start.unwrap(), b.end.unwrap());
            // 已按 start 排序，b_start >= a_start；若 b_start >= a_end 则后续都不重叠。
            if b_start >= a_end {
                break;
            }
            out.push(Conflict {
                item_a: i,
                item_b: j,
                message: format!(
                    "“{}”({}-{})与“{}”({}-{})时段重叠",
                    a.task_name,
                    a_start.format("%H:%M"),
                    a_end.format("%H:%M"),
                    b.task_name,
                    b_start.format("%H:%M"),
                    b_end.format("%H:%M")
                ),
            });
        }
    }
    out
}
```

- [ ] **Step 3: 更新 scheduler.rs 测试中的辅助函数 task()**

`task()` 辅助函数当前用 `priority: 0`，改为 `priority_level: PriorityLevel::default()`。在 scheduler.rs 测试模块顶部加 import：
```rust
use dailyplan_domain::task::PriorityLevel;
```
把测试里的 `task()` 函数的 `priority: 0` 改为 `priority_level: PriorityLevel::Normal`。

- [ ] **Step 4: 加新测试覆盖无时段任务**

在 scheduler.rs 测试模块加：
```rust
#[test]
fn untimed_task_produces_one_item_at_end() {
    let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
    let timed = task(
        1,
        "晨跑",
        Frequency::Daily { times_per_day: 1 },
        vec![slot("06:30", "07:00")],
    );
    let untimed = task(
        2,
        "读书",
        Frequency::Daily { times_per_day: 1 },
        vec![], // 无 slots
    );
    let plan = build_day_plan(date, &[untimed, timed]);
    assert_eq!(plan.items.len(), 2);
    // 定时任务在前，无时段在后
    assert_eq!(plan.items[0].task_name, "晨跑");
    assert_eq!(plan.items[1].task_name, "读书");
    assert!(plan.items[1].start.is_none());
}

#[test]
fn untimed_tasks_sorted_by_priority_desc() {
    let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
    let low = Task {
        id: 1, name: "低".into(), description: None,
        frequency: Frequency::Daily { times_per_day: 1 },
        slots: vec![], priority_level: PriorityLevel::Low, active: true,
    };
    let urgent = Task {
        id: 2, name: "急".into(), description: None,
        frequency: Frequency::Daily { times_per_day: 1 },
        slots: vec![], priority_level: PriorityLevel::Urgent, active: true,
    };
    let plan = build_day_plan(date, &[low, urgent]);
    assert_eq!(plan.items[0].task_name, "急");
    assert_eq!(plan.items[1].task_name, "低");
}
```

- [ ] **Step 5: 运行 engine 测试**

Run: `cargo test -p dailyplan-engine`
Expected: 全部通过（原 11 + 新 2 = 13）

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/scheduler.rs crates/engine/src/conflict.rs
git commit -m "feat(engine): untimed tasks sort last by priority; conflict skips untimed"
```

---

## Task 4: 渲染数据适配（engine 层）

**Files:**
- Modify: `crates/engine/src/render.rs`

**Interfaces:**
- Consumes: `ChecklistItem` 新字段（Task 2）
- Produces: `PrintItem { time: Option<String>, ..., pending: bool }`

- [ ] **Step 1: 改 PrintItem 结构**

修改 `crates/engine/src/render.rs` 的 `PrintItem`：
```rust
#[derive(Debug, Serialize)]
pub struct PrintItem {
    pub time: Option<String>,   // None 表示无时段（PDF 留空）
    pub task_name: String,
    pub duration_min: u32,
    pub note: String,
    pub pending: bool,
}
```

- [ ] **Step 2: 改 to_print_data 适配新字段**

修改 `to_print_data` 的 items 映射：
```rust
items: plan
    .items
    .iter()
    .map(|it| PrintItem {
        time: match (it.start, it.end) {
            (Some(s), Some(e)) => Some(format!("{}-{}", s.format("%H:%M"), e.format("%H:%M"))),
            _ => None,
        },
        task_name: it.task_name.clone(),
        duration_min: it.duration_min,
        note: String::new(),
        pending: it.pending,
    })
    .collect(),
```

- [ ] **Step 3: 运行 engine 测试**

Run: `cargo test -p dailyplan-engine`
Expected: 全部通过（render::tests::print_data_basic 需确认仍 OK）

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/render.rs
git commit -m "feat(engine): PrintItem with Option<time> and pending field"
```

---

## Task 5: DB 适配 PriorityLevel + Custom（src-tauri）

**Files:**
- Modify: `src-tauri/src/db.rs`

**Interfaces:**
- Consumes: `PriorityLevel`（Task 1）、`Frequency::Custom`（Task 1）
- Produces: db.rs 在行边界 i32 ↔ PriorityLevel 转换

- [ ] **Step 1: 改 row_to_task 的 priority 转换**

修改 `src-tauri/src/db.rs` 的 `row_to_task`，把：
```rust
let priority: i32 = row.get("priority")?;
```
改为：
```rust
let priority_rank: i32 = row.get("priority")?;
```
并把构造 `Task` 处的 `priority` 改为：
```rust
priority_level: dailyplan_domain::PriorityLevel::from_rank(priority_rank),
```

- [ ] **Step 2: 改 insert_task / update_task 的 priority 写入**

把 insert_task 里：
```rust
task.priority,
```
改为：
```rust
task.priority_level.rank(),
```

同样改 update_task 里的 `task.priority` → `task.priority_level.rank()`。

- [ ] **Step 3: 更新现有 db 测试适配 priority_level**

测试里的 `priority: 5` 改为 `priority_level: PriorityLevel::High`，`priority: 9` 改为 `priority_level: PriorityLevel::Urgent`，`priority: 0` 改为 `priority_level: PriorityLevel::Low`。

`insert_and_list` 测试的断言 `assert_eq!(all[0].priority, 5)` 改为 `assert_eq!(all[0].priority_level, PriorityLevel::High)`。

`update_changes_fields` 的 `t.priority = 9` 改为 `t.priority_level = PriorityLevel::Urgent`，断言改 `assert_eq!(all[0].priority_level, PriorityLevel::Urgent)`。

`frequency_roundtrip_all_variants` 测试里 `priority: 0` 改为 `priority_level: PriorityLevel::Low`。同时给 variants 加一个 Custom：
```rust
Frequency::Custom {
    dates: vec![
        NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
        NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
    ],
},
```

测试模块顶部加 `use dailyplan_domain::PriorityLevel;`。

- [ ] **Step 4: 运行 db 测试**

Run: `cargo test -p dailyplan`
Expected: 全部通过（4 个原测试 + Custom 往返新增）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat(db): PriorityLevel i32 conversion + Custom frequency roundtrip"
```

---

## Task 6: print_day 改为接收 items（src-tauri）

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/printing.rs`

**Interfaces:**
- Consumes: `PrintItem`（Task 4）
- Produces: `print_day(app, items: Vec<PrintItemInput>)` —— 前端传标记后的 items

- [ ] **Step 1: 在 render.rs 加 PrintItemInput 类型（前端→后端的输入类型）**

修改 `crates/engine/src/render.rs`，加一个前端传入用的类型（与 PrintItem 类似但反序列化）：
```rust
/// 前端传给 print_day 的单个 item（已标记 pending）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintItemInput {
    pub time: Option<String>,
    pub task_name: String,
    pub duration_min: u32,
    pub pending: bool,
}
```

加一个新函数把 PrintItemInput 转成 PrintData（重新排序：非 pending 在前按原序，pending 在后）：
```rust
/// 用前端传入的 items（已标记 pending）构造 PrintData。
/// pending 的 items 重排到末尾。
pub fn to_print_data_from_items(items: Vec<PrintItemInput>, opts: &RenderOptions) -> PrintData {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let weekday_cn = match today.weekday() {
        chrono::Weekday::Mon => "周一",
        chrono::Weekday::Tue => "周二",
        chrono::Weekday::Wed => "周三",
        chrono::Weekday::Thu => "周四",
        chrono::Weekday::Fri => "周五",
        chrono::Weekday::Sat => "周六",
        chrono::Weekday::Sun => "周日",
    };
    // 重排：非 pending 在前，pending 在后
    let mut sorted: Vec<&PrintItemInput> = items.iter().collect();
    sorted.sort_by_key(|it| it.pending);

    PrintData {
        title: opts.title.clone(),
        date: today.format("%Y-%m-%d").to_string(),
        weekday_cn: weekday_cn.to_string(),
        items: sorted
            .iter()
            .map(|it| PrintItem {
                time: it.time.clone(),
                task_name: it.task_name.clone(),
                duration_min: it.duration_min,
                note: String::new(),
                pending: it.pending,
            })
            .collect(),
        conflicts: Vec::new(),
        with_review: opts.with_review,
    }
}
```

- [ ] **Step 2: 改 commands.rs 的 print_day 签名**

修改 `src-tauri/src/commands.rs`，把 print_day 从 `(app, date)` 改为 `(app, items)`：
```rust
#[tauri::command]
pub async fn print_day(
    app: tauri::AppHandle,
    items: Vec<dailyplan_engine::render::PrintItemInput>,
) -> AppResult<String> {
    let pdf_path = crate::printing::print_day(&app, items).await?;
    Ok(pdf_path.to_string_lossy().to_string())
}
```

- [ ] **Step 3: 改 printing.rs 的 print_day 实现**

修改 `src-tauri/src/printing.rs`，把 `print_day(app, date_str)` 改为 `print_day(app, items)`，不再查 DB/调度：
```rust
pub async fn print_day(
    app: &AppHandle,
    items: Vec<dailyplan_engine::render::PrintItemInput>,
) -> Result<PathBuf, AppError> {
    let print_data = dailyplan_engine::render::to_print_data_from_items(
        items,
        &dailyplan_engine::render::RenderOptions::default(),
    );
    let template = dailyplan_engine::render::CHECKLIST_TYP;

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Db(format!("路径解析失败: {e}")))?;
    let print_dir = dir.join("print");
    std::fs::create_dir_all(&print_dir)
        .map_err(|e| AppError::Db(format!("创建目录失败: {e}")))?;

    let typ_path = print_dir.join("checklist.typ");
    let data_path = print_dir.join("data.json");
    let today_str = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let pdf_path = print_dir.join(format!("dailyplan-{today_str}.pdf"));

    let data_json = serde_json::to_string_pretty(&print_data)
        .map_err(|e| AppError::Db(format!("序列化失败: {e}")))?;
    std::fs::write(&typ_path, template)
        .map_err(|e| AppError::Db(format!("写模板失败: {e}")))?;
    std::fs::write(&data_path, data_json)
        .map_err(|e| AppError::Db(format!("写数据失败: {e}")))?;

    let shell = app.shell();
    let cmd = shell
        .sidecar("typst")
        .map_err(|e| AppError::Db(format!("启动 typst 失败: {e}")))?;
    let (mut rx, _child) = cmd
        .args([
            "compile",
            "--font-path",
            "/System/Library/Fonts:/Library/Fonts:~/Library/Fonts",
            typ_path.to_str().unwrap(),
            pdf_path.to_str().unwrap(),
        ])
        .spawn()
        .map_err(|e| AppError::Db(format!("spawn typst 失败: {e}")))?;

    let mut stderr_output = String::new();
    let mut exit_code: Option<i32> = None;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stderr(line) => {
                stderr_output.push_str(&String::from_utf8_lossy(&line));
                stderr_output.push('\n');
            }
            CommandEvent::Terminated(status) => {
                exit_code = status.code;
            }
            _ => {}
        }
    }

    if let Some(code) = exit_code {
        if code != 0 {
            return Err(AppError::Db(format!(
                "typst 编译失败 (code {code}): {stderr_output}"
            )));
        }
    }

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(pdf_path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| AppError::Db(format!("打开 PDF 失败: {e}")))?;

    Ok(pdf_path)
}
```

去掉 printing.rs 里原来的 `use dailyplan_engine::render::{to_print_data, RenderOptions};` 和 `use chrono::NaiveDate;`（不再需要）。

- [ ] **Step 4: 编译验证 src-tauri**

Run: `cargo check -p dailyplan`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/render.rs src-tauri/src/commands.rs src-tauri/src/printing.rs
git commit -m "feat(backend): print_day receives items from frontend (supports pending)"
```

---

## Task 7: Typst 模板 pending 行样式

**Files:**
- Modify: `crates/engine/templates/checklist.typ`

**Interfaces:**
- Consumes: `PrintItem.pending`（Task 4）

- [ ] **Step 1: 表格行根据 pending 变灰**

修改 `crates/engine/templates/checklist.typ` 的表格数据行，把每个 cell 根据 `it.pending` 加灰色背景。把：
```typst
..data.items.map(it => (
    it.time,
    it.task_name,
    if it.duration_min > 0 [ #it.duration_min 分 ] else [],
    checkbox(),
    [],
)).flatten(),
```
改为（pending 行的每个 cell 加 fill + text 换灰色）：
```typst
..data.items.map(it => {
    let cell-fill = if it.pending { luma(230) } else { white }
    let cell-text(fill) = table.cell(fill: fill)
    (
        if it.time == none { cell-text(cell-fill)[] } else { cell-text(cell-fill)[#it.time] },
        cell-text(cell-fill)[#it.task_name],
        if it.duration_min > 0 { cell-text(cell-fill)[ #it.duration_min 分 ] } else { cell-text(cell-fill)[] },
        cell-text(cell-fill)[#checkbox()],
        cell-text(cell-fill)[],
    )
}).flatten(),
```

注意：Typst 里 `none` 与字符串比较要用 `== none`（`it.time` 现在是 `Option`，serde 到 Typst 是 `none`/字符串）。

- [ ] **Step 2: 用假数据本地验证 Typst 编译**

把模板拷到 `/tmp/typst_test/checklist.typ`，造一个含 pending item 的 data.json：
```bash
cp crates/engine/templates/checklist.typ /tmp/typst_test/
cat > /tmp/typst_test/data.json <<'EOF'
{"title":"每日计划表","date":"2026-07-05","weekday_cn":"周六","with_review":true,
 "items":[
   {"time":"06:30-07:00","task_name":"晨跑","duration_min":30,"note":"","pending":false},
   {"time":null,"task_name":"读书","duration_min":0,"note":"","pending":false},
   {"time":null,"task_name":"复盘","duration_min":0,"note":"","pending":true}
 ],"conflicts":[]}
EOF
src-tauri/binaries/typst-aarch64-apple-darwin compile --font-path /System/Library/Fonts:/Library/Fonts /tmp/typst_test/checklist.typ /tmp/typst_test/out.pdf
```
Expected: 编译成功，无 error。pending 行（复盘）整行灰背景。

- [ ] **Step 3: Commit**

```bash
git add crates/engine/templates/checklist.typ
git commit -m "feat(typst): pending items rendered with grey background"
```

---

## Task 8: 前端 tauri.rs + app.rs 改 print_day 传 items

**Files:**
- Modify: `src/tauri.rs`
- Modify: `src/app.rs`
- Modify: `src/day_view.rs`

**Interfaces:**
- Consumes: 后端 `print_day(items)` 新签名（Task 6）
- Produces: 前端 `print_day(items)` 封装；DayView 暴露当前 items + pending 状态给 app

- [ ] **Step 1: 改 src/tauri.rs 的 print_day**

把：
```rust
pub async fn print_day(date: &str) -> Result<String, String> {
```
改为接收 items（前端构造好的 PrintItemInput 数组）：
```rust
#[derive(Serialize, Clone)]
pub struct PrintItemInput {
    pub time: Option<String>,
    pub task_name: String,
    pub duration_min: u32,
    pub pending: bool,
}

pub async fn print_day(items: Vec<PrintItemInput>) -> Result<String, String> {
    #[derive(Serialize)]
    struct Args {
        items: Vec<PrintItemInput>,
    }
    let args = serde_wasm_bindgen::to_value(&Args { items }).map_err(|e| e.to_string())?;
    let raw = invoke("print_day", args).await;
    serde_wasm_bindgen::from_value::<String>(raw).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 改 DayView 把 pending_ids 暴露给 on_print**

修改 `src/day_view.rs`，加 pending_ids 信号，并把 on_print 的签名从 `Fn(String)` 改为接收完整的 items 列表。

在 DayView 组件内加：
```rust
let pending_ids: RwSignal<std::collections::HashSet<i64>> = RwSignal::new(Default::default());
```

把 on_print 参数类型从 `impl Fn(String)` 改为：
```rust
on_print: impl Fn(Vec<crate::tauri::PrintItemInput>) + Send + Sync + 'static,
```

打印按钮的 on:click 改为构造 items（从当前 plan + pending_ids）：
```rust
<button class="primary" on:click=move |_| {
    if let Some(Ok(ref p)) = plan.get() {
        let items: Vec<crate::tauri::PrintItemInput> = p.items.iter().map(|it| {
            crate::tauri::PrintItemInput {
                time: match (it.start, it.end) {
                    (Some(s), Some(e)) => Some(format!("{}-{}", s.format("%H:%M"), e.format("%H:%M"))),
                    _ => None,
                },
                task_name: it.task_name.clone(),
                duration_min: it.duration_min,
                pending: pending_ids.get().contains(&it.task_id),
            }
        }).collect();
        on_print.with_value(|f| f(items));
    }
}>"🖨 打印"</button>
```

- [ ] **Step 3: 改 app.rs 的 on_print**

修改 `src/app.rs` 的 on_print，从 `move |date_str: String|` 改为 `move |items: Vec<crate::tauri::PrintItemInput>|`：
```rust
let on_print = move |items: Vec<crate::tauri::PrintItemInput>| {
    spawn_local(async move {
        match crate::tauri::print_day(items).await {
            Ok(_path) => {
                if let Some(w) = web_sys::window() {
                    let _ = w.alert_with_message("已生成 PDF 并打开，可在 Preview 中按 Cmd+P 打印");
                }
            }
            Err(e) => {
                if let Some(w) = web_sys::window() {
                    let _ = w.alert_with_message(&format!("打印失败: {e}"));
                }
            }
        }
    });
};
```

DayView 调用处改：
```rust
<DayView date on_print={move |items: Vec<crate::tauri::PrintItemInput>| on_print(items)} />
```

- [ ] **Step 4: 编译验证前端**

Run: `cargo check -p dailyplan-ui --target wasm32-unknown-unknown`
Expected: 编译通过（可能有 unused warning）

- [ ] **Step 5: Commit**

```bash
git add src/tauri.rs src/app.rs src/day_view.rs
git commit -m "feat(frontend): print_day receives items with pending state"
```

---

## Task 9: DayView 待定标记 UI（双显示）

**Files:**
- Modify: `src/day_view.rs`
- Modify: `styles.css`

**Interfaces:**
- Consumes: pending_ids 信号（Task 8）
- Produces: 主表格行变灰+划线 + 末尾待定区副本

- [ ] **Step 1: 主表格行加 pending class + 待定复选框**

修改 `src/day_view.rs` 的 `render_plan` 函数。

主表格的每行 `<tr>` 加 class 条件，并加一个"标记为待定"的复选框列。把 items 映射改为：
```rust
{items.iter().map(|(time, name, dur, task_id)| {
    let is_pending = move || pending_ids_read.get().contains(task_id);
    view! {
        <tr class:pending=is_pending>
            <td>{time.clone()}</td>
            <td>{name.clone()}</td>
            <td>{dur.clone()}</td>
            <td><input type="checkbox" /></td>
            <td>
                <label class="pending-toggle">
                    <input type="checkbox" prop:checked=is_pending
                        on:change=move |ev| {
                            let checked = event_target_checked(&ev);
                            pending_ids_write.update(|s| {
                                if checked { s.insert(*task_id); } else { s.remove(task_id); }
                            });
                        }/>
                    "待定"
                </label>
            </td>
        </tr>
    }
}).collect::<Vec<_>>()}
```

注意：`render_plan` 是自由函数，pending_ids 信号要作为参数传入。改 `render_plan` 签名：
```rust
fn render_plan(
    p: &DayPlan,
    pending_ids_read: ReadSignal<std::collections::HashSet<i64>>,
    pending_ids_write: RwSignal<std::collections::HashSet<i64>>,
) -> AnyView
```

但 RwSignal 同时是 Read+Write，简化为传一个 `RwSignal`：
```rust
fn render_plan(p: &DayPlan, pending_ids: RwSignal<std::collections::HashSet<i64>>) -> AnyView
```
调用处（plan.get() 的 match 分支）传 `pending_ids`。

items 元组要从 `(String, String, String)` 扩展为含 task_id：`(String, String, String, i64)`。修改构造 items 的 map：
```rust
let items: Vec<(String, String, String, i64)> = p.items.iter().map(|it| {
    (
        match (it.start, it.end) {
            (Some(s), Some(e)) => format!("{}-{}", s.format("%H:%M"), e.format("%H:%M")),
            _ => "随时".into(),
        },
        it.task_name.clone(),
        if it.duration_min > 0 { format!("{} 分", it.duration_min) } else { String::new() },
        it.task_id,
    )
}).collect();
```

- [ ] **Step 2: 末尾追加待定区**

在 `render_plan` 的表格之后、`item-count` 之前，加待定区（如果 pending_ids 非空）：
```rust
{move || {
    let pending = pending_ids.get();
    if pending.is_empty() { return view! { <span></span> }.into_any(); }
    let pending_items: Vec<&(String, String, String, i64)> = items.iter()
        .filter(|(_, _, _, id)| pending.contains(id))
        .collect();
    // 待定区按优先级排序——但 items 元组没存 priority。简化：按原序。
    view! {
        <div class="pending-section">
            <h4>"待定"</h4>
            <ul>
                {pending_items.iter().map(|(time, name, dur, _)| view! {
                    <li>{name.clone()} {if dur.is_empty() { String::new() } else { format!("（{}）", dur) }}</li>
                }).collect::<Vec<_>>()}
            </ul>
        </div>
    }.into_any()
}}
```

- [ ] **Step 3: 改 render_plan 调用处传 pending_ids**

DayView 组件里 `match plan.get()` 的 `Some(Ok(p)) => render_plan(&p)` 改为 `render_plan(&p, pending_ids)`。

- [ ] **Step 4: 加 CSS 样式**

修改 `styles.css`，加：
```css
/* 待定标记 */
tr.pending td { color: #999; text-decoration: line-through; background: #fafafa; }
.pending-toggle { display: inline-flex; align-items: center; gap: 0.3em; font-size: 0.8em; color: #888; cursor: pointer; }
.pending-toggle input { width: auto; }

/* 待定区 */
.pending-section {
    margin-top: 1em;
    padding: 0.6em 0.9em;
    border: 1px dashed #ccc;
    border-radius: 4px;
    background: #fafafa;
}
.pending-section h4 { margin: 0 0 0.4em; font-size: 0.9em; color: #888; }
.pending-section ul { margin: 0; padding-left: 1.2em; color: #666; font-size: 0.88em; }
```

- [ ] **Step 5: 编译验证 + trunk build**

Run:
```bash
cargo check -p dailyplan-ui --target wasm32-unknown-unknown
trunk build
```
Expected: 编译通过，dist 生成

- [ ] **Step 6: Commit**

```bash
git add src/day_view.rs styles.css
git commit -m "feat(frontend): pending toggle with grey+strikethrough + bottom pending section"
```

---

## Task 10: TaskEditor 优先级 select + 无时段开关

**Files:**
- Modify: `src/task_editor.rs`

**Interfaces:**
- Consumes: `PriorityLevel`（Task 1）、Task 字段变更

- [ ] **Step 1: EditorState 字段改造**

修改 `src/task_editor.rs` 的 EditorState：
- `priority: i32` → `priority_level: String`（存 snake_case 标签，转换在 to_task/from_task）
- 加 `untimed: bool`
- 加 `custom_dates: Vec<chrono::NaiveDate>`

```rust
pub struct EditorState {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub freq_kind: String,
    pub times_per_day: u32,
    pub weekdays: [bool; 7],
    pub every_days: u32,
    pub interval_start: String,
    pub once_date: String,
    pub custom_dates: Vec<chrono::NaiveDate>,
    pub slots: Vec<(String, String)>,
    pub priority_level: String,
    pub untimed: bool,
}
```

`Default`：
```rust
priority_level: "normal".into(),
untimed: false,
custom_dates: Vec::new(),
```

- [ ] **Step 2: 改 from_task / to_task**

`from_task`：
- `priority_level: match t.priority_level { PriorityLevel::Urgent => "urgent", ... }` 或者更简洁：用 `t.priority_level` 的 serde。但 EditorState 用 String，最简：
```rust
priority_level: match t.priority_level {
    dailyplan_domain::PriorityLevel::Urgent => "urgent".into(),
    dailyplan_domain::PriorityLevel::High => "high".into(),
    dailyplan_domain::PriorityLevel::Normal => "normal".into(),
    dailyplan_domain::PriorityLevel::Low => "low".into(),
},
untimed: t.slots.is_empty(),
custom_dates: if let Frequency::Custom { dates } = &t.frequency { dates.clone() } else { Vec::new() },
```

`to_task`：
- priority_level 从 String 解析：
```rust
let priority_level = match self.priority_level.as_str() {
    "urgent" => dailyplan_domain::PriorityLevel::Urgent,
    "high" => dailyplan_domain::PriorityLevel::High,
    "low" => dailyplan_domain::PriorityLevel::Low,
    _ => dailyplan_domain::PriorityLevel::Normal,
};
```
- 频率加 Custom 分支：
```rust
"custom" => {
    if self.custom_dates.is_empty() {
        return Err("请至少选择一个日期".into());
    }
    let mut dates = self.custom_dates.clone();
    dates.sort();
    dates.dedup();
    Frequency::Custom { dates }
}
```
- slots 处理加 untimed 分支：
```rust
let slots = if self.untimed {
    vec![]
} else {
    let mut slots = Vec::new();
    for (s, e) in &self.slots {
        // ...原校验...
    }
    slots
};
```

把 `priority: self.priority` 改为 `priority_level`。

- [ ] **Step 3: UI - 优先级 select 替换 number input**

把编辑器里的优先级 `<input type="number">` 替换为 `<select>`：
```rust
<label>"优先级"
    <select on:change=move |ev| state.update(|s| s.priority_level = event_target_value(&ev))>
        <option value="urgent" selected=move || state.get().priority_level == "urgent">"紧急"</option>
        <option value="high" selected=move || state.get().priority_level == "high">"重要"</option>
        <option value="normal" selected=move || state.get().priority_level == "normal">"一般"</option>
        <option value="low" selected=move || state.get().priority_level == "low">"可选"</option>
    </select>
</label>
```

- [ ] **Step 4: UI - 无时段复选框**

在时段 fieldset 前加：
```rust
<label class="untimed-toggle">
    <input type="checkbox" prop:checked=move || state.get().untimed
        on:change=move |ev| state.update(|s| {
            s.untimed = event_target_checked(&ev);
            if !s.untimed && s.slots.is_empty() {
                s.slots.push(("07:00".into(), "07:30".into()));
            }
        })/>
    "无固定时间（随时完成）"
</label>
```

把时段 fieldset 用 `{move || !state.get().untimed}.then(|| view! { ... })` 包起来（untimed 时隐藏）。

- [ ] **Step 5: freq_kind select 加 custom option**

在频率 select 加：
```rust
<option value="custom" selected=move || state.get().freq_kind == "custom">"指定日期"</option>
```

并在 freq_kind 的 match 加 custom 分支（日历组件在 Task 11 实现，这里先占位提示）：
```rust
"custom" => Some(view! {
    <div class="calendar-placeholder">
        <p>"日历组件将在下一步接入（任务 11）"</p>
        <p>"已选 " {move || state.get().custom_dates.len()} " 个日期"</p>
    </div>
}.into_any()),
```

- [ ] **Step 6: 编译验证**

Run: `cargo check -p dailyplan-ui --target wasm32-unknown-unknown`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add src/task_editor.rs
git commit -m "feat(editor): priority select + untimed toggle + custom freq placeholder"
```

---

## Task 11: 日历组件（内联月历网格）

**Files:**
- Create: `src/calendar.rs`
- Modify: `src/task_editor.rs`（接入日历）
- Modify: `src/main.rs`（声明模块）

**Interfaces:**
- Consumes: `weekday_to_index`（Task 1，pub(crate)）
- Produces: `<Calendar selected: RwSignal<Vec<NaiveDate>> />` 组件

- [ ] **Step 1: 写 src/calendar.rs**

```rust
//! 内联月历网格组件：多选日期。

use chrono::{Datelike, NaiveDate, Weekday};
use leptos::prelude::*;

/// 把 chrono Weekday 转为 周一=0..周日=6 的索引。
/// 复制自 domain::task::weekday_to_index（那里是 pub(crate)，跨 crate 不能直接用）。
fn weekday_to_index(d: Weekday) -> usize {
    match d {
        Weekday::Mon => 0, Weekday::Tue => 1, Weekday::Wed => 2,
        Weekday::Thu => 3, Weekday::Fri => 4, Weekday::Sat => 5, Weekday::Sun => 6,
    }
}

/// 生成某月的 6×7 网格日期（含前后月补齐）。
fn month_grid(year: i32, month: u32) -> Vec<Option<NaiveDate>> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let leading = weekday_to_index(first.weekday());
    let mut days: Vec<Option<NaiveDate>> = (0..leading).map(|_| None).collect();
    let mut d = first;
    while d.month() == month {
        days.push(Some(d));
        d = d.succ_opt().unwrap();
    }
    while days.len() % 7 != 0 {
        days.push(None);
    }
    days
}

/// 月历组件。selected 是外部持有的选中日期信号。
#[component]
pub fn Calendar(selected: RwSignal<Vec<NaiveDate>>) -> impl IntoView {
    let view_year = RwSignal::new(chrono::Local::now().date_naive().year());
    let view_month = RwSignal::new(chrono::Local::now().date_naive().month());

    let prev_month = move || {
        view_month.update(|m| {
            if *m == 1 { *m = 12; view_year.update(|y| *y -= 1); }
            else { *m -= 1; }
        });
    };
    let next_month = move || {
        view_month.update(|m| {
            if *m == 12 { *m = 1; view_year.update(|y| *y += 1); }
            else { *m += 1; }
        });
    };

    const HEADERS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

    view! {
        <div class="calendar">
            <div class="calendar-nav">
                <button on:click=move |_| prev_month()>"‹"</button>
                <span>{move || format!("{} 年 {} 月", view_year.get(), view_month.get())}</span>
                <button on:click=move |_| next_month()>"›"</button>
            </div>
            <div class="calendar-grid">
                {HEADERS.iter().map(|h| view! { <div class="calendar-cell header">{h}</div> }).collect::<Vec<_>>()}
                {move || {
                    let grid = month_grid(view_year.get(), view_month.get());
                    let sel = selected.get();
                    grid.into_iter().map(|d_opt| {
                        match d_opt {
                            None => view! { <div class="calendar-cell empty"></div> }.into_any(),
                            Some(d) => {
                                let is_selected = sel.contains(&d);
                                let d_for_click = d;
                                view! {
                                    <button class="calendar-cell" class:selected=is_selected
                                        on:click=move |_| {
                                            selected.update(|s| {
                                                if let Some(pos) = s.iter().position(|x| *x == d_for_click) {
                                                    s.remove(pos);
                                                } else {
                                                    s.push(d_for_click);
                                                    s.sort();
                                                    s.dedup();
                                                }
                                            });
                                        }>
                                        {d.day()}
                                    </button>
                                }.into_any()
                            }
                        }
                    }).collect::<Vec<_>>().into_any()
                }}
            </div>
            <div class="calendar-summary">
                {move || format!("已选 {} 个日期", selected.get().len())}
                <button on:click=move |_| selected.update(|s| s.clear())>"清空"</button>
            </div>
        </div>
    }.into_any()
}
```

- [ ] **Step 2: main.rs 声明 calendar 模块**

修改 `src/main.rs`，加：
```rust
mod calendar;
```

- [ ] **Step 3: task_editor.rs 接入日历**

把 Task 10 里的 calendar-placeholder 替换为真实日历组件。custom 分支改为：
```rust
"custom" => {
    let dates_signal = RwSignal::new(state.get().custom_dates.clone());
    // 双向同步：dates_signal 变 → state.custom_dates
    // 但 Leptos 0.8 简化做法：用 Effect 同步
    Effect::new(move || {
        let d = dates_signal.get();
        state.update(|s| s.custom_dates = d.clone());
    });
    Some(view! {
        <div>
            <crate::calendar::Calendar selected={dates_signal} />
        </div>
    }.into_any())
}
```

注意：Effect 在每次 render_plan 重建时会重复注册，可能有性能问题。更稳妥是用单个 dates_signal 持久化在 EditorState 外。但 MVP 先这样，后续优化。

- [ ] **Step 4: 加日历 CSS**

在 `styles.css` 末尾加：
```css
/* 日历 */
.calendar { font-size: 0.82em; }
.calendar-nav {
    display: flex; justify-content: space-between; align-items: center;
    margin-bottom: 0.4em;
}
.calendar-nav span { font-weight: 600; }
.calendar-nav button { padding: 0.2em 0.6em; }
.calendar-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
}
.calendar-cell {
    text-align: center;
    padding: 0.3em 0;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: #fff;
    font-size: 0.85em;
    cursor: pointer;
}
.calendar-cell.header { font-weight: 600; background: #f7f8fa; cursor: default; }
.calendar-cell.empty { border: none; background: transparent; cursor: default; }
.calendar-cell.selected { background: var(--primary); color: #fff; border-color: var(--primary); }
.calendar-summary {
    display: flex; justify-content: space-between; align-items: center;
    margin-top: 0.4em; color: #666;
}
.calendar-summary button { font-size: 0.8em; padding: 0.2em 0.6em; }
```

- [ ] **Step 5: 编译验证 + trunk build**

Run:
```bash
cargo check -p dailyplan-ui --target wasm32-unknown-unknown
trunk build
```
Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add src/calendar.rs src/main.rs src/task_editor.rs styles.css
git commit -m "feat(editor): inline calendar for Custom frequency date picking"
```

---

## Task 12: TaskList 优先级徽章 + Custom 标签

**Files:**
- Modify: `src/task_list.rs`

**Interfaces:**
- Consumes: `PriorityLevel::label_cn()`（Task 1）、`Frequency::Custom`（Task 1）

- [ ] **Step 1: 改 freq_label 加 Custom 分支**

修改 `src/task_list.rs` 的 `freq_label`，在 Once 之后加：
```rust
Frequency::Custom { dates } => {
    if dates.is_empty() {
        "从不".into()
    } else if dates.len() == 1 {
        format!("{} 当天", dates[0].format("%Y-%m-%d"))
    } else {
        format!("指定 {} 天 ({} 起)", dates.len(), dates.first().unwrap().format("%Y-%m-%d"))
    }
}
```

- [ ] **Step 2: 改 priority 显示为徽章**

把 TaskList 里构造 `priority_str` 的部分（`if t.priority != 0`）改为用 label_cn：
```rust
let priority_label = t.priority_level.label_cn();
let priority_class = match t.priority_level {
    dailyplan_domain::PriorityLevel::Urgent => "pri-urgent",
    dailyplan_domain::PriorityLevel::High => "pri-high",
    dailyplan_domain::PriorityLevel::Normal => "pri-normal",
    dailyplan_domain::PriorityLevel::Low => "pri-low",
};
```

把 `<span class="task-priority">{priority_str}</span>` 改为：
```rust
{move || if priority_label != "一般" {
    Some(view! { <span class=move || priority_class.clone()>" 优先级:" {priority_label}</span> })
} else { None }}
```

实际由于闭包捕获，简化为预先判断：
```rust
let show_priority = t.priority_level != dailyplan_domain::PriorityLevel::Normal;
```
view 里：
```rust
{show_priority.then(|| view! {
    <span class=priority_class>{format!("优先级:{}", priority_label)}</span>
})}
```

- [ ] **Step 3: 加优先级徽章 CSS**

styles.css 加：
```css
.pri-urgent { color: #d93025; font-weight: 600; }
.pri-high { color: #e8a33d; font-weight: 600; }
.pri-normal { color: #999; }
.pri-low { color: #bbb; }
```

- [ ] **Step 4: 编译验证 + trunk build**

Run:
```bash
cargo check -p dailyplan-ui --target wasm32-unknown-unknown
trunk build
```
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src/task_list.rs styles.css
git commit -m "feat(frontend): priority badges + Custom freq label in task list"
```

---

## Task 13: 全量验证 + 手动验收

**Files:** 无（验证任务）

- [ ] **Step 1: 全量测试**

Run: `cargo test --workspace`
Expected: 全部通过（domain 9 + engine 13 + db 4 = 26）

- [ ] **Step 2: 全量编译**

Run:
```bash
cargo check -p dailyplan
cargo check -p dailyplan-ui --target wasm32-unknown-unknown
trunk build
```
Expected: 全部通过

- [ ] **Step 3: 手动验收清单**

启动应用 `cargo tauri dev`，逐项验证：
1. 新建无时段任务（勾选"无固定时间"）→ 当天打卡表末尾出现"随时"
2. 新建任务选优先级"紧急" → TaskList 显示红色徽章
3. 新建任务频率选"指定日期" → 日历显示，点几个日期 → 仅那些日期出现该任务
4. 当日视图某行点"待定"复选框 → 原位变灰+划线 + 末尾出现副本
5. 点打印 → PDF 中原位不出现待定项，末尾出现（灰色背景）
6. 编辑现有任务（原 priority 数据）→ 优先级 select 默认选中正确级别

- [ ] **Step 4: 清理旧数据（如需要）**

如果旧任务的 priority 值导致显示异常，删掉 DB 重建：
```bash
rm ~/Library/Application\ Support/com.dailyplan.app/dailyplan.db
```
重启应用会自动建新库。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: full verification of task scheduling enhancements"
```

---

## Self-Review 结果

**Spec 覆盖检查：**
- ✅ 日历选日期：Task 1（domain）+ Task 11（UI）
- ✅ 无时段任务：Task 2（ChecklistItem）+ Task 3（scheduler）+ Task 4（render）+ Task 10（editor）
- ✅ 优先级文字级别：Task 1（domain）+ Task 5（db）+ Task 10（editor）+ Task 12（list）
- ✅ 待定标记：Task 6（print_day）+ Task 8（前端 print）+ Task 9（DayView UI）+ Task 7（Typst）

**类型一致性：**
- `PriorityLevel` 在 Task 1 定义，Task 2/3/5/10/12 一致使用
- `ChecklistItem` 字段 Task 2 定义，Task 3/4 一致使用
- `PrintItemInput` Task 6 定义（engine::render），Task 8（前端 tauri.rs）镜像定义

**已知风险点（实现时注意）：**
1. Task 9 的 render_plan 闭包捕获 pending_ids——RwSignal 是 Copy-like，传值即可
2. Task 11 的 Effect 同步 dates_signal ↔ state.custom_dates 可能重复注册——MVP 可接受，后续优化
3. Task 7 的 Typst `it.time == none` 语法需实际验证（serde Option 到 Typst）
