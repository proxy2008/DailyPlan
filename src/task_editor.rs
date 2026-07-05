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
    pub freq_kind: String, // "daily" | "weekly" | "interval" | "once" | "custom"
    pub times_per_day: u32,
    pub weekdays: [bool; 7],
    pub every_days: u32,
    pub interval_start: String, // YYYY-MM-DD
    pub once_date: String,      // YYYY-MM-DD
    pub custom_dates: Vec<chrono::NaiveDate>,
    pub slots: Vec<(String, String)>, // (start "HH:MM", end "HH:MM")
    pub priority_level: String, // snake_case tag: urgent|high|normal|low
    pub untimed: bool,
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
            custom_dates: Vec::new(),
            slots: vec![("07:00".into(), "07:30".into())],
            priority_level: "normal".into(),
            untimed: false,
        }
    }
}

impl EditorState {
    /// 从已有 Task 填充编辑器。
    pub fn from_task(t: &Task) -> Self {
        use dailyplan_domain::PriorityLevel;
        let (freq_kind, times_per_day, weekdays, every_days, interval_start, once_date, custom_dates) =
            match &t.frequency {
                Frequency::Daily { times_per_day } => {
                    ("daily", *times_per_day, [true; 7], 1, String::new(), String::new(), Vec::new())
                }
                Frequency::Weekly { weekdays } => {
                    ("weekly", 1, *weekdays, 1, String::new(), String::new(), Vec::new())
                }
                Frequency::Interval { every_days, start } => {
                    ("interval", 1, [true; 7], *every_days, start.format("%Y-%m-%d").to_string(), String::new(), Vec::new())
                }
                Frequency::Once { date } => {
                    ("once", 1, [true; 7], 1, String::new(), date.format("%Y-%m-%d").to_string(), Vec::new())
                }
                Frequency::Custom { dates } => {
                    ("custom", 1, [true; 7], 1, String::new(), String::new(), dates.clone())
                }
            };
        let priority_level = match t.priority_level {
            PriorityLevel::Urgent => "urgent",
            PriorityLevel::High => "high",
            PriorityLevel::Normal => "normal",
            PriorityLevel::Low => "low",
        };
        let slots: Vec<(String, String)> = t
            .slots
            .iter()
            .map(|s| (s.start.format("%H:%M").to_string(), s.end.format("%H:%M").to_string()))
            .collect();
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
            custom_dates,
            untimed: slots.is_empty(),
            slots,
            priority_level: priority_level.into(),
        }
    }

    /// 校验并转成 Task（id=0 表示新建）。
    pub fn to_task(&self) -> Result<Task, String> {
        use dailyplan_domain::PriorityLevel;
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
            "custom" => {
                if self.custom_dates.is_empty() {
                    return Err("请至少选择一个日期".into());
                }
                let mut dates = self.custom_dates.clone();
                dates.sort();
                dates.dedup();
                Frequency::Custom { dates }
            }
            other => return Err(format!("未知频率类型: {other}")),
        };
        // 无固定时间：跳过 slot 解析，slots = []。
        let slots = if self.untimed {
            Vec::new()
        } else {
            let mut parsed = Vec::new();
            for (s, e) in &self.slots {
                let start = parse_time(s).ok_or_else(|| format!("起始时间格式错误: {s}"))?;
                let end = parse_time(e).ok_or_else(|| format!("结束时间格式错误: {e}"))?;
                if end <= start {
                    return Err(format!("时段 {s}-{e} 结束须晚于开始"));
                }
                parsed.push(TimeSlot { start, end });
            }
            if parsed.is_empty() {
                return Err("请至少添加一个时间段，或勾选「无固定时间」".into());
            }
            parsed
        };
        let priority_level = match self.priority_level.as_str() {
            "urgent" => PriorityLevel::Urgent,
            "high" => PriorityLevel::High,
            "low" => PriorityLevel::Low,
            _ => PriorityLevel::Normal,
        };
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
            priority_level,
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
    // 日历选中的日期：提到组件顶层，只创建一次，避免在渲染闭包内重复创建导致 Effect 死循环。
    let dates_signal = RwSignal::new(state.get().custom_dates.clone());
    // slots 独立信号：输入时只更新 slots_signal，不触发整个 state 重渲染（否则 DOM 重建导致输入法 panic）。
    let slots_signal: RwSignal<Vec<(String, String)>> = RwSignal::new(state.get().slots.clone());

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if saving.get() {
            return;
        }
        // 保存前把独立信号同步进 state
        state.update(|s| {
            s.custom_dates = dates_signal.get();
            s.slots = slots_signal.get();
        });
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

            <label>"要求（可选，执行标准/注意事项，打印到备注列）"
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
                    <option value="custom" selected=move || state.get().freq_kind == "custom">"指定日期"</option>
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
                    "custom" => {
                        Some(view! {
                            <div>
                                <crate::calendar::Calendar selected={dates_signal} />
                            </div>
                        }.into_any())
                    }
                    _ => None,
                }}
            </fieldset>

            <div class="untimed-toggle">
                <input type="checkbox" id="untimed" prop:checked=move || state.get().untimed
                    on:change=move |ev| {
                        let checked = event_target_checked(&ev);
                        state.update(|s| s.untimed = checked);
                        if !checked && slots_signal.get().is_empty() {
                            slots_signal.update(|s| s.push(("07:00".into(), "07:30".into())));
                        }
                    }/>
                <label for="untimed">"无固定时间（随时完成）"</label>
            </div>

            // 时间段：始终渲染，untimed 时置灰禁用（避免勾选时 modal 高度突变）
            <fieldset class="slots" class:disabled=move || state.get().untimed>
                <legend>"时间段（硬绑定）"</legend>
                // 渲染时段输入框——用 slots 的当前快照渲染一次，
                // 输入只更新 slots_signal，不触发本闭包重跑（避免 DOM 重建导致输入法 panic）。
                {move || {
                    let snapshot = slots_signal.get();
                    let rows: Vec<_> = snapshot.iter().enumerate().map(|(i, (sv, ev))| {
                        let ss = slots_signal;
                        view! {
                            <div class="slot-row" data-slot-index=i>
                                <input type="time" value=sv.clone()
                                    on:input=move |ev| {
                                        ss.update(|s| { if i < s.len() { s[i].0 = event_target_value(&ev); } });
                                    } />
                                <span>"-"</span>
                                <input type="time" value=ev.clone()
                                    on:input=move |ev| {
                                        ss.update(|s| { if i < s.len() { s[i].1 = event_target_value(&ev); } });
                                    } />
                                <button type="button"
                                    on:click=move |_| ss.update(|s| { if s.len() > 1 { s.remove(i); } })>
                                    "✕"
                                </button>
                            </div>
                        }.into_any()
                    }).collect();
                    rows
                }}
                <button type="button" class="add-slot"
                    on:click=move |_| slots_signal.update(|s| s.push(("08:00".into(), "08:30".into())))>
                    "+ 添加时段"
                </button>
            </fieldset>

            <label>"优先级"
                <select on:change=move |ev| state.update(|s| s.priority_level = event_target_value(&ev))>
                    <option value="urgent" selected=move || state.get().priority_level == "urgent">"紧急"</option>
                    <option value="high" selected=move || state.get().priority_level == "high">"重要"</option>
                    <option value="normal" selected=move || state.get().priority_level == "normal">"一般"</option>
                    <option value="low" selected=move || state.get().priority_level == "low">"可选"</option>
                </select>
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
