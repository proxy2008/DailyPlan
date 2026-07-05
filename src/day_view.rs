//! 当日打卡表视图：选日期、显示当天计划、冲突告警。
//!
//! 用 RwSignal<Option<Result<DayPlan,String>>> + Effect 手动管理异步加载，
//! 避免 Resource 对 !Send future 的要求（wasm_bindgen future 含 Rc，非 Send）。

use std::collections::HashSet;

use chrono::NaiveDate;
use dailyplan_domain::DayPlan;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// 当日打卡表视图。
/// `date` 控制日期；`tasks` 追踪任务列表变化自动刷新；`on_print` 打印回调。
#[component]
pub fn DayView(
    date: RwSignal<String>,
    tasks: ReadSignal<Vec<dailyplan_domain::Task>>,
    on_print: impl Fn(String, Vec<crate::tauri::PrintItemInput>) + Send + Sync + 'static,
) -> impl IntoView {
    // 当天计划状态：None=加载中，Some(Ok)=成功，Some(Err)=失败。
    let plan = RwSignal::new(None::<Result<DayPlan, String>>);
    let on_print = StoredValue::new(on_print);

    // 待定标记：保存被标记为"待定"的 task_id 集合。
    let pending_ids = RwSignal::new(HashSet::<i64>::new());

    // date 或 tasks 变化时重新加载（增删改任务后自动刷新当天打卡表）
    Effect::new(move || {
        let d = date.get();
        let _ = tasks.get(); // 追踪 tasks 信号变化
        plan.set(None);
        spawn_local(async move {
            let result = crate::tauri::generate_day(&d).await;
            plan.set(Some(result));
        });
    });

    let go = move |delta: i64| {
        let cur = NaiveDate::parse_from_str(&date.get(), "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive());
        let next = cur + chrono::Duration::days(delta);
        date.set(next.format("%Y-%m-%d").to_string());
    };

    let today = move || {
        date.set(chrono::Local::now().date_naive().format("%Y-%m-%d").to_string());
    };

    view! {
        <div class="day-view">
            <div class="day-toolbar">
                <button on:click=move |_| go(-1)>"‹ 前一天"</button>
                <input type="date" prop:value=move || date.get()
                    on:input=move |ev| date.set(event_target_value(&ev)) />
                <button on:click=move |_| go(1)>"后一天 ›"</button>
                <button on:click=move |_| today()>"今天"</button>
                <button class="primary" on:click=move |_| {
                    if let Some(Ok(ref p)) = plan.get() {
                        let d = date.get();
                        let pending_set = pending_ids.get();
                        let items: Vec<crate::tauri::PrintItemInput> = p.items.iter().map(|it| {
                            crate::tauri::PrintItemInput {
                                time: match (it.start, it.end) {
                                    (Some(s), Some(e)) => Some(format!("{}-{}", s.format("%H:%M"), e.format("%H:%M"))),
                                    _ => None,
                                },
                                task_name: it.task_name.clone(),
                                duration_min: it.duration_min,
                                pending: pending_set.contains(&it.task_id),
                                note: it.requirement.clone(),
                            }
                        }).collect();
                        on_print.with_value(|f| f(d, items));
                    }
                }>"🖨 打印"</button>
            </div>

            {move || match plan.get() {
                None => view! { <p>"生成中…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">"加载失败: " {e}</p> }.into_any(),
                Some(Ok(p)) => render_plan(&p, pending_ids),
            }}
        </div>
    }.into_any()
}

/// 把 DayPlan 渲染成视图（空态/冲突/表格 + 待定区）。
fn render_plan(p: &DayPlan, pending_ids: RwSignal<HashSet<i64>>) -> AnyView {
    if p.items.is_empty() && p.conflicts.is_empty() {
        return view! {
            <div class="day-plan">
                <h3>{p.date.format("%Y-%m-%d").to_string()}</h3>
                <p class="empty">"今日暂无计划任务"</p>
            </div>
        }.into_any();
    }

    let date_str = p.date.format("%Y-%m-%d").to_string();
    let conflicts: Vec<String> = p.conflicts.iter().map(|c| c.message.clone()).collect();
    let items: Vec<(String, String, String, i64)> = p
        .items
        .iter()
        .map(|it| {
            (
                match (it.start, it.end) {
                    (Some(s), Some(e)) => format!("{}-{}", s.format("%H:%M"), e.format("%H:%M")),
                    _ => String::new(),
                },
                it.task_name.clone(),
                if it.duration_min > 0 {
                    format!("{}min", it.duration_min)
                } else {
                    String::new()
                },
                it.task_id,
            )
        })
        .collect();
    let items_len = items.len();

    view! {
        <div class="day-plan">
            <h3>{date_str}</h3>

            {(!conflicts.is_empty()).then(|| {
                let cs = conflicts.clone();
                view! {
                    <div class="conflicts">
                        <strong>"⚠ 时段冲突"</strong>
                        <ul>
                            {cs.iter().map(|c| view! { <li>{c.clone()}</li> }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                }
            })}

            <table class="checklist">
                <thead>
                    <tr>
                        <th>"时间"</th>
                        <th>"任务"</th>
                        <th>"时长"</th>
                        <th>"完成"</th>
                        <th>"待定"</th>
                    </tr>
                </thead>
                <tbody>
                    {items.iter().map(|(time, name, dur, task_id)| {
                        let task_id = *task_id;
                        let is_pending = move || pending_ids.get().contains(&task_id);
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
                                                pending_ids.update(|s| {
                                                    if checked { s.insert(task_id); } else { s.remove(&task_id); }
                                                });
                                            }/>
                                        "待定"
                                    </label>
                                </td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
            <p class="item-count">"共 " {items_len} " 项"</p>

            {move || {
                let pending = pending_ids.get();
                if pending.is_empty() { return view! { <span></span> }.into_any(); }
                let pending_items: Vec<&(String, String, String, i64)> = items.iter()
                    .filter(|(_, _, _, id)| pending.contains(id))
                    .collect();
                view! {
                    <div class="pending-section">
                        <h4>"待定"</h4>
                        <ul>
                            {pending_items.iter().map(|(_, name, dur, _)| {
                                let suffix = if dur.is_empty() {
                                    String::new()
                                } else {
                                    format!("（{}）", dur)
                                };
                                view! {
                                    <li>{name.clone()} {suffix}</li>
                                }
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                }.into_any()
            }}
        </div>
    }.into_any()
}
