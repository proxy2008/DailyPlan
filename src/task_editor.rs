//! 任务编辑器组件：新建/编辑任务的表单。

use dailyplan_domain::{Frequency, Task, TimeSlot};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri;

/// 把 HH:MM 字符串解析成 NaiveTime，失败返回 None。
fn parse_time(s: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// 编辑器状态。editing 为 Some 时是编辑现有任务，None 时是新建。
#[derive(Clone)]
pub struct EditorState {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub freq_kind: String, // "daily" | "weekly" | "interval" | "once"
    pub times_per_day: u32,
    pub weekdays: [bool; 7],
    pub every_days: u32,
    pub interval_start: String, // YYYY-MM-DD
    pub once_date: String,      // YYYY-MM-DD
    pub slots: Vec<(String, String)>, // (start "HH:MM", end "HH:MM")
    pub priority: i32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            description: String::new(),
            freq_kind: "daily".into(),
            times_per_day: 1,
            weekdays: [true; 7],
            every_days: 1,
            interval_start: chrono::Local::now().date_naive().format("%Y-%m-%d").to_string(),
            once_date: chrono::Local::now().date_naive().format("%Y-%m-%d").to_string(),
            slots: vec![("07:00".into(), "07:30".into())],
            priority: 0,
        }
    }
}

impl EditorState {
    /// 从已有 Task 填充编辑器。
    pub fn from_task(t: &Task) -> Self {
        let (freq_kind, times_per_day, weekdays, every_days, interval_start, once_date) =
            match &t.frequency {
                Frequency::Daily { times_per_day } => {
                    ("daily", *times_per_day, [true; 7], 1, String::new(), String::new())
                }
                Frequency::Weekly { weekdays } => {
                    ("weekly", 1, *weekdays, 1, String::new(), String::new())
                }
                Frequency::Interval { every_days, start } => {
                    ("interval", 1, [true; 7], *every_days, start.format("%Y-%m-%d").to_string(), String::new())
                }
                Frequency::Once { date } => {
                    ("once", 1, [true; 7], 1, String::new(), date.format("%Y-%m-%d").to_string())
                }
            };
        Self {
            id: Some(t.id),
            name: t.name.clone(),
            description: t.description.clone().unwrap_or_default(),
            freq_kind: freq_kind.into(),
            times_per_day,
            weekdays,
            every_days,
            interval_start,
            once_date,
            slots: t
                .slots
                .iter()
                .map(|s| (s.start.format("%H:%M").to_string(), s.end.format("%H:%M").to_string()))
                .collect(),
            priority: t.priority,
        }
    }

    /// 校验并转成 Task（id=0 表示新建）。
    pub fn to_task(&self) -> Result<Task, String> {
        if self.name.trim().is_empty() {
            return Err("任务名不能为空".into());
        }
        let frequency = match self.freq_kind.as_str() {
            "daily" => Frequency::Daily {
                times_per_day: self.times_per_day.max(1),
            },
            "weekly" => Frequency::Weekly {
                weekdays: self.weekdays,
            },
            "interval" => {
                let start = chrono::NaiveDate::parse_from_str(&self.interval_start, "%Y-%m-%d")
                    .map_err(|_| "间隔起始日期格式错误".to_string())?;
                Frequency::Interval {
                    every_days: self.every_days.max(1),
                    start,
                }
            }
            "once" => {
                let date = chrono::NaiveDate::parse_from_str(&self.once_date, "%Y-%m-%d")
                    .map_err(|_| "单次日期格式错误".to_string())?;
                Frequency::Once { date }
            }
            other => return Err(format!("未知频率类型: {other}")),
        };
        let mut slots = Vec::new();
        for (s, e) in &self.slots {
            let start = parse_time(s).ok_or_else(|| format!("起始时间格式错误: {s}"))?;
            let end = parse_time(e).ok_or_else(|| format!("结束时间格式错误: {e}"))?;
            if end <= start {
                return Err(format!("时段 {s}-{e} 结束须晚于开始"));
            }
            slots.push(TimeSlot { start, end });
        }
        Ok(Task {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            description: if self.description.trim().is_empty() {
                None
            } else {
                Some(self.description.trim().to_string())
            },
            frequency,
            slots,
            priority: self.priority,
            active: true,
        })
    }
}

const WEEKDAY_LABELS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

/// 任务编辑器组件。
/// `on_saved` 在保存成功后回调（通常刷新列表）。
#[component]
pub fn TaskEditor(
    initial: EditorState,
    on_saved: impl Fn() + Send + Sync + 'static,
    on_cancel: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let state = RwSignal::new(initial);
    let saving = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);
    let on_saved = StoredValue::new(on_saved);
    let on_cancel = StoredValue::new(on_cancel);

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if saving.get() {
            return;
        }
        let snap = state.get();
        let task = match snap.to_task() {
            Ok(t) => t,
            Err(e) => {
                error_msg.set(Some(e));
                return;
            }
        };
        saving.set(true);
        error_msg.set(None);
        let is_new = task.id == 0;
        spawn_local(async move {
            let res = if is_new {
                tauri::create_task(task).await.map(|_| ())
            } else {
                tauri::update_task(task).await
            };
            saving.set(false);
            match res {
                Ok(()) => on_saved.with_value(|f| f()),
                Err(e) => error_msg.set(Some(e)),
            }
        });
    };

    view! {
        <form class="editor" on:submit=save>
            <h3>{move || state.get().id.map(|_| "编辑任务").unwrap_or("新建任务")}</h3>

            <label>"任务名"
                <input type="text" prop:value=move || state.get().name
                    on:input=move |ev| state.update(|s| s.name = event_target_value(&ev)) />
            </label>

            <label>"描述（可选）"
                <input type="text" prop:value=move || state.get().description
                    on:input=move |ev| state.update(|s| s.description = event_target_value(&ev)) />
            </label>

            <fieldset class="freq">
                <legend>"频率"</legend>
                <select on:change=move |ev| state.update(|s| s.freq_kind = event_target_value(&ev))>
                    <option value="daily" selected=move || state.get().freq_kind == "daily">"每天"</option>
                    <option value="weekly" selected=move || state.get().freq_kind == "weekly">"每周指定日"</option>
                    <option value="interval" selected=move || state.get().freq_kind == "interval">"每 N 天"</option>
                    <option value="once" selected=move || state.get().freq_kind == "once">"单次"</option>
                </select>

                {move || match state.get().freq_kind.as_str() {
                    "daily" => Some(view! {
                        <label>"每天次数"
                            <input type="number" min="1" prop:value=move || state.get().times_per_day
                                on:input=move |ev| state.update(|s| s.times_per_day = event_target_value(&ev).parse().unwrap_or(1)) />
                        </label>
                    }.into_any()),
                    "weekly" => Some(view! {
                        <div class="weekdays">
                            {WEEKDAY_LABELS.iter().enumerate().map(|(i, lbl)| {
                                let checked = move || state.get().weekdays[i];
                                let label_text = lbl.to_string();
                                view! {
                                    <label class="weekday">
                                        <input type="checkbox" prop:checked=checked
                                            on:change=move |ev| state.update(|s| s.weekdays[i] = event_target_checked(&ev)) />
                                        {label_text}
                                    </label>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()),
                    "interval" => Some(view! {
                        <div>
                            <label>"每"
                                <input type="number" min="1" prop:value=move || state.get().every_days
                                    on:input=move |ev| state.update(|s| s.every_days = event_target_value(&ev).parse().unwrap_or(1)) />
                                " 天"
                            </label>
                            <label>"起始"
                                <input type="date" prop:value=move || state.get().interval_start.clone()
                                    on:input=move |ev| state.update(|s| s.interval_start = event_target_value(&ev)) />
                            </label>
                        </div>
                    }.into_any()),
                    "once" => Some(view! {
                        <label>"日期"
                            <input type="date" prop:value=move || state.get().once_date.clone()
                                on:input=move |ev| state.update(|s| s.once_date = event_target_value(&ev)) />
                        </label>
                    }.into_any()),
                    _ => None,
                }}
            </fieldset>

            <fieldset class="slots">
                <legend>"时间段（硬绑定）"</legend>
                {move || state.get().slots.iter().enumerate().map(|(i, _)| {
                    let start_val = move || state.get().slots[i].0.clone();
                    let end_val = move || state.get().slots[i].1.clone();
                    view! {
                        <div class="slot-row">
                            <input type="time" prop:value=start_val
                                on:input=move |ev| state.update(|s| s.slots[i].0 = event_target_value(&ev)) />
                            <span>"-"</span>
                            <input type="time" prop:value=end_val
                                on:input=move |ev| state.update(|s| s.slots[i].1 = event_target_value(&ev)) />
                            <button type="button"
                                on:click=move |_| state.update(|s| { if s.slots.len() > 1 { s.slots.remove(i); } })>
                                "✕"
                            </button>
                        </div>
                    }
                }).collect::<Vec<_>>()}
                <button type="button" class="add-slot"
                    on:click=move |_| state.update(|s| s.slots.push(("08:00".into(), "08:30".into())))>
                    "+ 添加时段"
                </button>
            </fieldset>

            <label>"优先级（数字越大越优先）"
                <input type="number" prop:value=move || state.get().priority
                    on:input=move |ev| state.update(|s| s.priority = event_target_value(&ev).parse().unwrap_or(0)) />
            </label>

            {move || error_msg.get().map(|e| view! { <p class="error">{e}</p> })}

            <div class="editor-actions">
                <button type="submit" disabled=move || saving.get()>
                    {move || if saving.get() { "保存中…" } else { "保存" }}
                </button>
                <button type="button" on:click=move |_| on_cancel.with_value(|f| f())>"取消"</button>
            </div>
        </form>
    }.into_any()
}
