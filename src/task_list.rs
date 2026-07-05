//! 任务列表组件：展示所有任务，支持编辑/删除。

use dailyplan_domain::{Frequency, Task};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::task_editor::EditorState;

/// 把频率转成中文简述。
fn freq_label(f: &Frequency) -> String {
    match f {
        Frequency::Daily { times_per_day } => {
            if *times_per_day == 1 {
                "每天".into()
            } else {
                format!("每天 {} 次", times_per_day)
            }
        }
        Frequency::Weekly { weekdays } => {
            const LABELS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];
            let days: Vec<&str> = weekdays
                .iter()
                .enumerate()
                .filter_map(|(i, on)| if *on { Some(LABELS[i]) } else { None })
                .collect();
            if days.len() == 7 {
                "每天".into()
            } else if days.is_empty() {
                "从不".into()
            } else {
                format!("每周{}", days.join("、"))
            }
        }
        Frequency::Interval { every_days, .. } => format!("每 {} 天", every_days),
        Frequency::Once { date } => format!("{} 单次", date.format("%Y-%m-%d")),
        Frequency::Custom { dates } => {
            if dates.is_empty() {
                "从不".into()
            } else if dates.len() == 1 {
                format!("{} 当天", dates[0].format("%Y-%m-%d"))
            } else {
                format!(
                    "指定 {} 天 ({} 起)",
                    dates.len(),
                    dates.first().unwrap().format("%Y-%m-%d")
                )
            }
        }
    }
}

/// 任务列表组件。
/// `tasks` 是当前任务列表信号；`on_edit` 点击编辑时回调（传 task 副本）；
/// `on_refresh` 删除后刷新。
#[component]
pub fn TaskList(
    tasks: ReadSignal<Vec<Task>>,
    on_edit: impl Fn(EditorState) + Send + Sync + 'static,
    on_refresh: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let on_edit = StoredValue::new(on_edit);
    let on_refresh = StoredValue::new(on_refresh);

    view! {
        <div class="task-list">
            <h2>"我的任务（" {move || tasks.get().len()} "）"</h2>
            <div class="cards">
                <For each=move || tasks.get() key=|t| t.id let(t)>
                    {move || {
                        let edit_state = EditorState::from_task(&t);
                        let name = t.name.clone();
                        let id = t.id;
                        let freq = freq_label(&t.frequency);
                        let slots = t.slots.iter()
                            .map(|s| format!("{}-{}", s.start.format("%H:%M"), s.end.format("%H:%M")))
                            .collect::<Vec<_>>().join("   ");
                        let pl = t.priority_level.label_cn().to_string();
                        let pc = match t.priority_level {
                            dailyplan_domain::PriorityLevel::Urgent => "pri-urgent",
                            dailyplan_domain::PriorityLevel::High => "pri-high",
                            dailyplan_domain::PriorityLevel::Normal => "pri-normal",
                            dailyplan_domain::PriorityLevel::Low => "pri-low",
                        }.to_string();
                        let show_pri = t.priority_level != dailyplan_domain::PriorityLevel::Normal;
                        view! {
                            <div class="task-card">
                                <div class="task-card-main">
                                    <span class="task-name">{name.clone()}</span>
                                    <span class="task-freq">{freq}</span>
                                    <span class="task-slots">{slots}</span>
                                    {show_pri.then(|| view! { <span class=pc.clone()>{format!("优先级:{}", pl)}</span> })}
                                </div>
                                <div class="task-card-actions">
                                    <button type="button" on:click=move |_| on_edit.with_value(|f| f(edit_state.clone()))>"编辑"</button>
                                    <button type="button" class="danger" on:click=move |_| {
                                        let id = id;
                                        spawn_local(async move {
                                            match crate::tauri::delete_task(id).await {
                                                Ok(()) => { on_refresh.with_value(|f| f()); }
                                                Err(e) => {
                                                    web_sys::console::error_1(&format!("删除失败: {e}").into());
                                                    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                                                        doc.set_title(&format!("❌删除失败: {e}"));
                                                    }
                                                }
                                            }
                                        });
                                    }>"删除"</button>
                                </div>
                            </div>
                        }
                    }}
                </For>
            </div>
            {move || if tasks.get().is_empty() {
                Some(view! { <p class="empty">"还没有任务，点击「新建任务」开始添加。"</p> })
            } else {
                None
            }}
        </div>
    }.into_any()
}
