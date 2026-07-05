//! 任务卡片渲染（共享）：被 TaskManage 页复用。
//! 事件由 app.rs 全局委托处理，这里只渲染。

use dailyplan_domain::{Frequency, PriorityLevel, Task};
use leptos::prelude::*;

/// 把频率转成中文简述。
pub fn freq_label(f: &Frequency) -> String {
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

/// 频率的大类（用于筛选下拉）。返回字符串 tag，方便匹配。
pub fn freq_kind(f: &Frequency) -> &'static str {
    match f {
        Frequency::Daily { .. } => "daily",
        Frequency::Weekly { .. } => "weekly",
        Frequency::Interval { .. } => "interval",
        Frequency::Once { .. } => "once",
        Frequency::Custom { .. } => "custom",
    }
}

/// 优先级对应的 CSS class。
pub fn priority_class(p: PriorityLevel) -> &'static str {
    match p {
        PriorityLevel::Urgent => "pri-urgent",
        PriorityLevel::High => "pri-high",
        PriorityLevel::Normal => "pri-normal",
        PriorityLevel::Low => "pri-low",
    }
}

/// 渲染单张任务卡片。`confirming` 控制内联删除确认 UI。
pub fn render_card(t: &Task, confirming: RwSignal<Option<i64>>) -> AnyView {
    let name = t.name.clone();
    let id = t.id;
    let freq = freq_label(&t.frequency);
    let slots = t
        .slots
        .iter()
        .map(|s| format!("{}-{}", s.start.format("%H:%M"), s.end.format("%H:%M")))
        .collect::<Vec<_>>()
        .join("   ");
    let req = t.description.clone().unwrap_or_default();
    let pl = t.priority_level.label_cn().to_string();
    let pc = priority_class(t.priority_level).to_string();
    let show_pri = t.priority_level != PriorityLevel::Normal;
    view! {
        <div class="task-card">
            <div class="task-card-main">
                <span class="task-name">{name.clone()}</span>
                <span class="task-freq">{freq}</span>
                <span class="task-slots">{slots}</span>
                {show_pri.then(|| view! { <span class=pc.clone()>{format!("优先级:{}", pl)}</span> })}
                {(!req.is_empty()).then(|| view! { <span class="task-req">{format!("要求:{}", req)}</span> })}
            </div>
            <div class="task-card-actions">
                <button type="button" class="btn-task-action"
                    data-action="edit" data-task-id=id>"编辑"</button>
                <button type="button" class="btn-task-action danger"
                    data-action="delete" data-task-id=id
                    class:hidden=move || confirming.get() == Some(id)>"删除"</button>
                <span class="confirm-inline"
                    class:hidden=move || confirming.get() != Some(id)>
                    "删除？"
                    <button type="button" class="btn-task-action danger"
                        data-action="confirm-delete" data-task-id=id>"是"</button>
                    <button type="button" class="btn-task-action"
                        data-action="cancel-delete" data-task-id=id>"否"</button>
                </span>
            </div>
        </div>
    }.into_any()
}
