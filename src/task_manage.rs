//! 任务管理页：工具条（搜索 + 频率/优先级筛 + 排序）+ 分组/平铺渲染。
//!
//! 事件由 app.rs 全局委托处理（data-action="edit"/"delete"/...）。

use dailyplan_domain::{Frequency, PriorityLevel, Task};
use leptos::prelude::*;

use crate::task_list::{freq_kind, render_card};

/// 排序键。
#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Priority,
    Name,
    Created,
}

/// 升降序。
#[derive(Clone, Copy, PartialEq, Eq)]
enum SortDir {
    Asc,
    Desc,
}

/// 优先级分组用：从高到低 4 桶。
const PRI_GROUPS: [PriorityLevel; 4] = [
    PriorityLevel::Urgent,
    PriorityLevel::High,
    PriorityLevel::Normal,
    PriorityLevel::Low,
];

/// 任务管理页组件。
#[component]
pub fn TaskManage(
    tasks: ReadSignal<Vec<Task>>,
    confirming: RwSignal<Option<i64>>,
    tasks_rev: RwSignal<u32>,
    on_create: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let start_create = StoredValue::new(on_create);
    // 工具条状态
    let search_kw = RwSignal::new(String::new());
    let freq_filter = RwSignal::new(None::<&'static str>); // None = 全部
    let pri_filter = RwSignal::new(None::<PriorityLevel>); // None = 全部
    let sort_key = RwSignal::new(SortKey::Priority);
    let sort_dir = RwSignal::new(SortDir::Desc);

    // 派生：搜索 → 频率筛 → 优先级筛 → 排序
    let filtered = Memo::new(move |_| {
        let _ = tasks_rev.get(); // 编辑后强制重算
        let kw = search_kw.get().to_lowercase();
        let freq = freq_filter.get();
        let pri = pri_filter.get();
        let mut v: Vec<Task> = tasks
            .get()
            .into_iter()
            .filter(|t| {
                if kw.is_empty() {
                    return true;
                }
                t.name.to_lowercase().contains(&kw)
                    || t
                        .description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&kw)
            })
            .filter(|t| freq.map_or(true, |f| freq_kind(&t.frequency) == f))
            .filter(|t| pri.map_or(true, |p| t.priority_level == p))
            .collect();

        let dir = sort_dir.get();
        match sort_key.get() {
            SortKey::Priority => v.sort_by(|a, b| {
                let cmp = b.priority_level.rank().cmp(&a.priority_level.rank());
                if dir == SortDir::Asc { cmp.reverse() } else { cmp }
            }),
            SortKey::Name => v.sort_by(|a, b| {
                let cmp = a.name.cmp(&b.name);
                if dir == SortDir::Asc { cmp } else { cmp.reverse() }
            }),
            // 创建时间：用 id 近似（id 越大越新）
            SortKey::Created => v.sort_by(|a, b| {
                let cmp = b.id.cmp(&a.id);
                if dir == SortDir::Asc { cmp.reverse() } else { cmp }
            }),
        }
        v
    });

    // 总数（不受筛选影响）+ 筛后数
    let total = Memo::new(move |_| tasks.get().len());

    view! {
        <div class="task-manage">
            // 页面头部：标题 + 新建按钮
            <div class="tasks-header">
                <div>
                    <h2>"任务管理"</h2>
                    <div class="page-subtitle">{move || format!("共 {} 个任务", total.get())}</div>
                </div>
                <button class="primary new-task-btn"
                    on:click=move |_| start_create.with_value(|f| f())>
                    "+ 新建任务"
                </button>
            </div>

            // 工具条
            <div class="toolbar-row">
                <input class="search-input" type="text" placeholder="🔍  搜索任务名或要求…"
                    prop:value=move || search_kw.get()
                    on:input=move |ev| search_kw.set(event_target_value(&ev)) />
                <div class="toolbar-divider"></div>
                <select class="filter-select"
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        freq_filter.set(match v.as_str() {
                            "daily" => Some("daily"),
                            "weekly" => Some("weekly"),
                            "interval" => Some("interval"),
                            "once" => Some("once"),
                            "custom" => Some("custom"),
                            _ => None,
                        });
                    }>
                    <option value="all">"全部频率"</option>
                    <option value="daily">"每天"</option>
                    <option value="weekly">"每周"</option>
                    <option value="interval">"间隔"</option>
                    <option value="once">"单次"</option>
                    <option value="custom">"指定日期"</option>
                </select>
                <select class="filter-select"
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        pri_filter.set(match v.as_str() {
                            "urgent" => Some(PriorityLevel::Urgent),
                            "high" => Some(PriorityLevel::High),
                            "normal" => Some(PriorityLevel::Normal),
                            "low" => Some(PriorityLevel::Low),
                            _ => None,
                        });
                    }>
                    <option value="all">"全部优先级"</option>
                    <option value="urgent">"紧急"</option>
                    <option value="high">"重要"</option>
                    <option value="normal">"一般"</option>
                    <option value="low">"可选"</option>
                </select>
                <select class="filter-select"
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        sort_key.set(match v.as_str() {
                            "name" => SortKey::Name,
                            "created" => SortKey::Created,
                            _ => SortKey::Priority,
                        });
                    }>
                    <option value="priority">"按优先级"</option>
                    <option value="name">"按名称"</option>
                    <option value="created">"按创建时间"</option>
                </select>
                <button class="sort-dir-btn" type="button"
                    title=move || if sort_dir.get() == SortDir::Desc { "当前：降序" } else { "当前：升序" }
                    on:click=move |_| {
                        sort_dir.update(|d| *d = if *d == SortDir::Desc { SortDir::Asc } else { SortDir::Desc });
                    }>
                    {move || if sort_dir.get() == SortDir::Desc { "↓" } else { "↑" }}
                </button>
                <span class="count-badge">
                    {move || {
                        let t = total.get();
                        let f = filtered.get().len();
                        if t == f { format!("共 {}", t) } else { format!("{}/{}", f, t) }
                    }}
                </span>
            </div>

            // 列表：分组 or 平铺
            <div class="cards-container">
                {move || {
                    let list = filtered.get();
                    if list.is_empty() {
                        let total_now = total.get();
                        return view! {
                            <div class="empty-state">
                                <div class="empty-icon">{if total_now == 0 { "📋" } else { "🔍" }}</div>
                                <div class="empty-title">{
                                    if total_now == 0 {
                                        "还没有任务".to_string()
                                    } else {
                                        "没有匹配的任务".to_string()
                                    }
                                }</div>
                                <div class="empty-hint">{
                                    if total_now == 0 {
                                        "点击右上角「新建任务」开始添加".to_string()
                                    } else {
                                        "试试调整搜索关键字或筛选条件".to_string()
                                    }
                                }</div>
                            </div>
                        }.into_any();
                    }
                    // 排序键 = 优先级 → 分组；否则平铺
                    if sort_key.get() == SortKey::Priority {
                        render_grouped(&list, confirming).into_any()
                    } else {
                        render_flat(&list, confirming, tasks_rev).into_any()
                    }
                }}
            </div>
        </div>
    }.into_any()
}

/// 按优先级分组渲染（4 桶，空桶不显示）。
fn render_grouped(list: &[Task], confirming: RwSignal<Option<i64>>) -> impl IntoView {
    let groups: Vec<(PriorityLevel, Vec<Task>)> = PRI_GROUPS
        .iter()
        .map(|p| {
            let items: Vec<Task> = list.iter().filter(|t| &t.priority_level == p).cloned().collect();
            (*p, items)
        })
        .collect();

    view! {
        <For each=move || groups.clone()
            key=|(p, _)| format!("{:?}", p)
            let(group)>
                {move || {
                    let (pri, items) = group.clone();
                    if items.is_empty() {
                        return view! { <span></span> }.into_any();
                    }
                    let label = format!("{}（{}）", pri.label_cn(), items.len());
                    let pcls = match pri {
                        PriorityLevel::Urgent => "group-header pri-urgent",
                        PriorityLevel::High => "group-header pri-high",
                        PriorityLevel::Normal => "group-header pri-normal",
                        PriorityLevel::Low => "group-header pri-low",
                    };
                    view! {
                        <div class="task-group">
                            <div class=pcls>{label}</div>
                            <div class="cards">
                                <For each=move || items.clone()
                                    key=|t| t.id
                                    let(t)>
                                        {move || render_card(&t, confirming)}
                                </For>
                            </div>
                        </div>
                    }.into_any()
                }}
        </For>
    }
}

/// 平铺渲染（按名称/创建时间排序时用）。
fn render_flat(
    list: &[Task],
    confirming: RwSignal<Option<i64>>,
    tasks_rev: RwSignal<u32>,
) -> impl IntoView {
    let list: Vec<Task> = list.to_vec();
    view! {
        <div class="cards">
            <For each=move || { let _ = tasks_rev.get(); list.clone() }
                key={let tr = tasks_rev; move |t| (tr.get(), t.id) } let(t)>
                    {move || render_card(&t, confirming)}
            </For>
        </div>
    }
}

// 兼容性：原 TaskList 已废弃，但保留 Frequency 引用避免 unused warning。
#[allow(dead_code)]
fn _freq_compat(_f: &Frequency) {}
