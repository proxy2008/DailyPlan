# 任务调度增强设计

> 日期：2026-07-04
> 范围：日历选日期频率 + 无时段任务 + 优先级文字级别 + 当日"待定"标记
> 不在范围：任务状态持久化（明确推迟到后续迭代）

## 背景

当前 MVP 的任务模型有几个限制：
- 频率只能用规则表达（每天/每周/每N天/单次），无法处理"7月5、8、12、19号要做"这种不规则日期
- 所有任务必须绑定时间段，但"读书/冥想/复盘"这类没有固定时间
- 优先级用 `i32` 表示，用户不知道"3"意味着什么
- 当日视图里无法临时把某项挪后（"待定"）

本次迭代解决这四个问题，但**不引入任务状态持久化**——"待定"是当日临时状态，刷新即丢。

## 总体设计

四个子需求共享底座改动（domain 模型、ChecklistItem 字段、调度排序、打印数据），必须协同设计。其中"无时段任务"和"优先级级别"耦合最深：无时段排序依赖 ChecklistItem 携带优先级字段。

---

## 子需求 1：日历选日期频率

### 数据模型

`Frequency` 新增变体：

```rust
pub enum Frequency {
    Daily { times_per_day: u32 },
    Weekly { weekdays: [bool; 7] },
    Interval { every_days: u32, start: NaiveDate },
    Once { date: NaiveDate },
    Custom { dates: Vec<NaiveDate> },  // 新增：用户手动选的日期，保持升序+去重
}
```

`Frequency::matches`：
```rust
Frequency::Custom { dates } => dates.binary_search(&date).is_ok(),
```
（构造时排序+去重，匹配用二分查找。）

### 存储

`frequency` 已是 JSON 文本列，新变体自动序列化为 `{"kind":"custom","params":{"dates":["2026-07-05",...]}}`。**无需 DB schema 改动，无需迁移。** 一个任务存一年日期约 3.6KB，对 SQLite 完全无压力。

### UI：内联月历网格

在 TaskEditor 的频率 `<select>` 加 `<option value="custom">指定日期</option>`。选中后，在 fieldset 内联展开一个月历组件：

```
‹  2026 年 7 月  ›
一 二 三 四 五 六 日
       1  2  3  4  5
 6  7  8  9 10 11 12
13 14 15 16 17 18 19
...
已选 4 个日期（最早 2026-07-05）  [清空]
```

- 手写 Leptos 组件，~100 行，零新依赖
- 6 周 × 7 列网格，每格一个 `<button>`，点击 toggle 该日期
- 已选日期高亮（如蓝色背景）
- 允许选过去日期（用户可能补录）
- 用 chrono 从 `NaiveDate::from_ymd_opt(year, month, 1)` + 月首 weekday 算偏移生成网格
- `weekday_to_index` 辅助函数当前是 `domain::task` 私有函数，改为 `pub(crate)` 让 UI 复用

### EditorState

新增字段：
```rust
pub custom_dates: Vec<chrono::NaiveDate>, // 升序+去重
```

toggle 辅助函数：
```rust
fn toggle_date(s: &mut EditorState, d: NaiveDate) {
    if let Some(pos) = s.custom_dates.iter().position(|x| *x == d) {
        s.custom_dates.remove(pos);
    } else {
        s.custom_dates.push(d);
        s.custom_dates.sort();
        s.custom_dates.dedup();
    }
}
```

`to_task` 校验非空（"请至少选择一个日期"），构造 `Frequency::Custom { dates }`。

### 显示

- TaskList 的 `freq_label`：`"指定 4 天 (2026-07-05 起)"`，单日时显示 `"2026-07-05 当天"`
- 打印模板：**无需改动**（Custom 任务只在命中日期产出 item，模板只渲染 items）

---

## 子需求 2：无时段任务

### 数据模型

复用 `slots: Vec<TimeSlot>` —— **空 slots 即代表无时段**。不新增 TaskKind 变体（避免到处 match）。

`ChecklistItem` 的时间字段改为 Option：
```rust
pub struct ChecklistItem {
    pub task_id: i64,
    pub task_name: String,
    pub start: Option<NaiveTime>,   // 原为 NaiveTime
    pub end: Option<NaiveTime>,     // 原为 NaiveTime
    pub duration_min: u32,          // 无时段时为 0
    pub priority: PriorityLevel,    // 新增（见子需求 3）
    pub pending: bool,              // 新增（见子需求 4）
}
```

### 调度器改动

`build_day_plan` 加分支处理空 slots：

```rust
let mut items: Vec<ChecklistItem> = tasks
    .iter()
    .filter(|t| t.active && t.frequency.matches(date))
    .flat_map(|t| {
        if t.slots.is_empty() {
            // 无时段任务：产出单个 untimed item
            vec![ChecklistItem {
                task_id: t.id,
                task_name: t.name.clone(),
                start: None, end: None,
                duration_min: 0,
                priority: t.priority_level,
                pending: false,
            }]
        } else {
            t.slots.iter().map(move |slot| ChecklistItem {
                task_id: t.id, task_name: t.name.clone(),
                start: Some(slot.start), end: Some(slot.end),
                duration_min: slot.duration_minutes(),
                priority: t.priority_level,
                pending: false,
            }).collect::<Vec<_>>()
        }
    })
    .collect();
```

**注意**：`pending` 字段后端始终设为 `false`——它是前端临时状态，不来自后端。

### 排序

```rust
items.sort_by(|a, b| {
    a.start.is_none().cmp(&b.start.is_none())      // 无时段 (true) 排最后
        .then_with(|| a.start.cmp(&b.start))        // 定时：早的在前
        .then_with(|| b.priority.rank().cmp(&a.priority.rank())) // 同时刻：优先级高在前
        .then_with(|| a.task_id.cmp(&b.task_id))    // 稳定 tiebreak
});
```

**确认的行为**：无时段任务**始终**排在所有定时任务之后，无论优先级高低。无时段任务之间按优先级降序排。

### 冲突检测

`detect_conflicts` 跳过无时段 item（无时间区间无从冲突）：
```rust
for j in (i + 1)..n {
    let a = &items[i];
    let b = &items[j];
    if a.start.is_none() || b.start.is_none() { continue; }  // 跳过无时段
    if b.start.unwrap() >= a.end.unwrap() { break; }
    // ... 记录冲突
}
```

### 渲染层改动

| 位置 | 改动 |
|---|---|
| `render.rs::PrintItem.time` | 改为 `Option<String>`，无时段时 `None`（PDF 该格留空） |
| `render.rs::to_print_data` | 无时段时 `time: None`，`duration_min: 0` |
| `day_view.rs` 表格 | 无时段时时间列显示"随时"或留空 |
| Typst 模板 | `it.time` 为 `none` 时 Typst 渲染为空，无需改模板逻辑 |

### EditorState

新增字段：
```rust
pub untimed: bool,  // 是否无固定时间
```

- `from_task`：`untimed: t.slots.is_empty()`
- `to_task`：`untimed` 为 true 时 `slots = vec![]`；为 false 时保持原校验（至少 1 个有效 slot）
- UI：时段 fieldset 上方加复选框"☐ 无固定时间（随时完成）"，勾选后隐藏 slots 区域

---

## 子需求 3：优先级文字级别

### 数据模型

新增枚举（与 Frequency 同文件）：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLevel {
    Urgent,   // 紧急
    High,     // 重要
    Normal,   // 一般（默认）
    Low,      // 可选
}

impl PriorityLevel {
    pub fn rank(&self) -> i32 {
        match self {
            Self::Urgent => 3,
            Self::High => 2,
            Self::Normal => 1,
            Self::Low => 0,
        }
    }
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

impl Default for PriorityLevel {
    fn default() -> Self { Self::Normal }
}
```

`Task` 字段变更：
```rust
// 原：pub priority: i32,
pub priority_level: PriorityLevel,
```

### 存储

**复用 `priority` INTEGER 列存 rank**（0-3）。db.rs 在行边界做 `i32 ↔ PriorityLevel` 转换：
- 读：`priority_level: PriorityLevel::from_rank(row.get::<_, i32>("priority")?)`
- 写：`task.priority_level.rank()`

`PriorityLevel::from_rank` 把整数映射回枚举（0→Low, 1→Normal, 2→High, 3→Urgent，越界值 clamp）。

### 数据迁移

**用户已确认：旧数据可以清空重建。** 不写数据映射迁移。但 `priority INTEGER NOT NULL DEFAULT 0` 的默认值 0 现在对应 `Low`——而新任务的默认应是 `Normal`(1)。处理方式：
- 加 V002 迁移：`ALTER TABLE tasks ALTER COLUMN priority SET DEFAULT 1;`（SQLite 3.37+ 支持）
- 或者：INSERT 时显式写 `priority_level.rank()`，不依赖列默认值（更稳）

推荐后者——db.rs 的 `insert_task` 显式传 `task.priority_level.rank() as i64`，不依赖 schema 默认值。

### UI

TaskEditor：把 `<input type="number">` 换成 `<select>`：
```html
<select>
  <option value="urgent">紧急</option>
  <option value="high">重要</option>
  <option value="normal" selected>一般</option>
  <option value="low">可选</option>
</select>
```

EditorState：`priority: i32` → `priority_level: PriorityLevel`（Copy 类型，信号更新廉价）。

TaskList：把 `"优先级 N"` 文本换成彩色小徽章，颜色按级别：
- 紧急：红
- 重要：橙
- 一般：灰（或不显示徽章）
- 可选：浅灰（或不显示）

打印 PDF：暂不加优先级列（避免改 5 列表格结构）。可选地在任务名前加一个小标记（如紧急加 `▶`）——**本期不做，留待后续**。

---

## 子需求 4：当日"待定"标记

这是当日视图的**临时状态**，不持久化、不写库、刷新即丢。

### 数据流

```
后端 generate_day → DayPlan { items: Vec<ChecklistItem { ..., pending: false }> }
                                    ↓
前端 DayView 收到，所有 pending 都是 false
                                    ↓
用户勾选某行"待定" → 前端本地信号更新该 item.pending = true
                                    ↓
重新渲染：原位变灰+划线 + 末尾追加副本
                                    ↓
用户点打印 → 前端把标记后的 items 传给 print_day（而不是重新让后端生成）
```

**关键改动**：`print_day` 当前是自己读 DB + 调度。要支持"待定"，必须改为**前端把已标记的 items 传给后端**，后端只负责渲染 PDF。

### 前端 DayView 改动

新增本地信号：
```rust
let pending_ids: RwSignal<HashSet<i64>> = RwSignal::new(HashSet::new());
```

每行加一个复选框"标记为待定"。勾选时 `pending_ids.update(|s| s.insert(item.task_id))`。

渲染逻辑：
1. **主表格**：所有 items 按原排序显示，但 `pending_ids.contains(&item.task_id)` 的行加 class `pending`（CSS 变灰+划线），复选框打勾
2. **末尾待定区**：再渲染一遍被标记的 items（按优先级排序），样式正常或加"待定"标记
3. 取消待定：在**原位置**点掉复选框 → `pending_ids.update(|s| s.remove(&id))`，末尾副本同步消失

### print_day 改动

签名从 `(app, date)` 改为 `(app, items: Vec<PrintItem>)`——前端把标记后的 items 直接传过来。后端不再查 DB 调度，只用传入的 items 渲染 PDF。

PDF 排序：
- 非 pending 的定时 items（按时间）
- 非 pending 的无时段 items（按优先级）
- pending 的 items（按优先级）—— **原位不出现，只在末尾**

`PrintItem` 加 `pending: bool` 字段。Typst 模板里 pending 行加灰色背景或斜体区分。

### Tauri 命令契约变更

```rust
// 原：
#[tauri::command]
async fn print_day(app: AppHandle, date: String) -> AppResult<String>

// 新：
#[tauri::command]
async fn print_day(app: AppHandle, items: Vec<PrintItemInput>) -> AppResult<String>
```

`PrintItemInput` 是前端传来的、已标记 pending 的 items（含 task_name/start/end/duration/pending）。

---

## 涉及文件清单

| 文件 | 改动 |
|---|---|
| `crates/domain/src/task.rs` | 加 `PriorityLevel` 枚举；`Frequency::Custom`；`Task.priority: i32` → `priority_level: PriorityLevel`；`weekday_to_index` 改 `pub(crate)` |
| `crates/domain/src/checklist.rs` | `ChecklistItem.start/end` 改 `Option`；加 `priority`、`pending` 字段 |
| `crates/engine/src/scheduler.rs` | 空 slots 分支；新排序键 |
| `crates/engine/src/conflict.rs` | 跳过无时段 item |
| `crates/engine/src/render.rs` | `PrintItem.time: Option<String>`；加 `pending: bool`；`print_day` 接收 items 而非 date |
| `crates/engine/templates/checklist.typ` | pending 行灰色样式（可选） |
| `src-tauri/src/db.rs` | `priority i32 ↔ PriorityLevel` 转换；`Frequency::Custom` 加到往返测试 |
| `src-tauri/src/commands.rs` | `print_day` 签名变更（接 items） |
| `src-tauri/src/printing.rs` | 用传入 items 渲染，不再查 DB |
| `src/task_editor.rs` | `untimed` 复选框；优先级 `<select>`；日历组件；`EditorState` 字段 |
| `src/task_list.rs` | 优先级徽章；`freq_label` 加 Custom 分支 |
| `src/day_view.rs` | `pending_ids` 信号；原位变灰+末尾副本；print 传 items |
| `src/tauri.rs` | `print_day` 封装改为传 items |
| `styles.css` | `.pending` 灰色+划线样式 |

## 测试策略

- **domain 单测**：`Frequency::Custom` 的 matches；`PriorityLevel::rank`/`from_rank` 往返
- **engine 单测**：无时段任务产出 untimed item；排序（定时在前无时段在后）；无时段不产生冲突；Custom 频率命中
- **db 单测**：PriorityLevel 往返；Custom 频率往返
- **手动验收**：
  1. 建一个无时段任务 → 当天打卡表末尾出现
  2. 建一个 Custom 频率任务，选几个日期 → 仅那些日期出现
  3. 优先级下拉选"紧急" → TaskList 显示红色徽章
  4. 当日视图勾选某行"待定" → 原位变灰+末尾出现副本
  5. 打印 → PDF 末尾出现待定项，原位不出现

## 不做的事

- 任务状态持久化（待定、完成等都不写库）
- 软时段自动调度
- PDF 加优先级列
- 历史/统计
