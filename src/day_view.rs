//! 当日打卡表视图：选日期、显示当天计划、冲突告警。
//!
//! 用 RwSignal<Option<Result<DayPlan,String>>> + Effect 手动管理异步加载，
//! 避免 Resource 对 !Send future 的要求（wasm_bindgen future 含 Rc，非 Send）。

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};
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
    on_print_days: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    // 当天计划状态：None=加载中，Some(Ok)=成功，Some(Err)=失败。
    let plan = RwSignal::new(None::<Result<DayPlan, String>>);
    let on_print = StoredValue::new(on_print);
    let on_print_days = StoredValue::new(on_print_days);

    // 待定标记：保存被标记为"待定"的 task_id 集合。
    let pending_ids = RwSignal::new(HashSet::<i64>::new());
    // 打印下拉菜单开关
    let print_menu_open = RwSignal::new(false);
    // 菜单定位（fixed 坐标，避免被 overflow 容器裁剪）
    let print_menu_pos = RwSignal::new((0i32, 0i32));

    // 触发单日打印：收集当前 plan + pending 标记
    let do_print_day = move || {
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
        print_menu_open.set(false);
    };

    // 打开菜单：记录按钮的视口坐标，菜单用 fixed 定位到按钮下方
    let open_print_menu = move |ev: leptos::ev::MouseEvent| {
        use wasm_bindgen::JsCast;
        // currentTarget 就是绑定 on:click 的按钮
        if let Some(ct) = ev.current_target() {
            let el: &web_sys::Element = ct.unchecked_ref();
            let rect = el.get_bounding_client_rect();
            print_menu_pos.set((rect.right() as i32, rect.bottom() as i32));
            print_menu_open.set(true);
        }
    };

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
            // 页面头部：日期标题 + 右侧导航/打印
            <div class="schedule-header">
                <div class="date-display">
                    {move || {
                        let d = date.get();
                        let nd = NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                            .unwrap_or_else(|_| chrono::Local::now().date_naive());
                        let weekday = match nd.weekday() {
                            chrono::Weekday::Mon => "周一",
                            chrono::Weekday::Tue => "周二",
                            chrono::Weekday::Wed => "周三",
                            chrono::Weekday::Thu => "周四",
                            chrono::Weekday::Fri => "周五",
                            chrono::Weekday::Sat => "周六",
                            chrono::Weekday::Sun => "周日",
                        };
                        let is_today = nd == chrono::Local::now().date_naive();
                        view! {
                            <div class="date-main">{nd.format("%m 月 %d 日").to_string()} <span style="font-size:0.6em; font-weight:400; color:var(--text-3); margin-left:0.4em">{weekday}</span></div>
                            <div class="date-sub">
                                {if is_today { "今天".to_string() } else { nd.format("%Y-%m-%d").to_string() }}
                                {" · 每日计划"}
                            </div>
                        }.into_any()
                    }}
                </div>
                <div class="date-nav">
                    <button class="icon-btn" title="前一天" on:click=move |_| go(-1)>"‹"</button>
                    <input type="date" prop:value=move || date.get()
                        on:input=move |ev| date.set(event_target_value(&ev)) />
                    <button class="icon-btn" title="后一天" on:click=move |_| go(1)>"›"</button>
                    <button on:click=move |_| today()>"今天"</button>
                    // 打印下拉菜单
                    <div class="print-dropdown">
                        <button class="primary" on:click=open_print_menu>"🖨 打印 ▾"</button>
                    </div>
                </div>
            </div>

            {move || match plan.get() {
                None => view! {
                    <div class="loading-state">"加载中…"</div>
                }.into_any(),
                Some(Err(e)) => view! {
                    <div class="error-state">"加载失败：" {e}</div>
                }.into_any(),
                Some(Ok(p)) => render_plan(&p, pending_ids),
            }}
        </div>

        // 打印菜单浮层：fixed 定位，脱离 .page 的 overflow 容器
        {move || if print_menu_open.get() {
            let (right, top) = print_menu_pos.get();
            Some(view! {
                <div class="print-menu-overlay" on:click=move |_| print_menu_open.set(false)>
                    <div class="print-menu"
                        style:top=format!("{}px", top + 6)
                        style:right=format!("{}px", window_width() - right)
                        on:click=move |ev| ev.stop_propagation()>
                        <button type="button" class="print-menu-item" on:click=move |_| do_print_day()>
                            "🖨 打印当天"
                        </button>
                        <button type="button" class="print-menu-item"
                            on:click=move |_| {
                                print_menu_open.set(false);
                                on_print_days.with_value(|f| f());
                            }>
                            "📅 打印多日…"
                        </button>
                    </div>
                </div>
            })
        } else { None }}
    }.into_any()
}

/// 当前视口宽度（用于 fixed 定位菜单的 right 计算）。
fn window_width() -> i32 {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .map(|f| f as i32)
        .unwrap_or(1280)
}

/// 把 DayPlan 渲染成视图（空态/冲突/卡片列表 + 待定区）。
fn render_plan(p: &DayPlan, pending_ids: RwSignal<HashSet<i64>>) -> AnyView {
    if p.items.is_empty() && p.conflicts.is_empty() {
        return view! {
            <div class="empty-state">
                <div class="empty-icon">"🗒"</div>
                <div class="empty-title">"今日暂无计划任务"</div>
                <div class="empty-hint">"去「任务管理」添加任务，或切换其他日期查看"</div>
            </div>
        }.into_any();
    }

    let conflicts: Vec<String> = p.conflicts.iter().map(|c| c.message.clone()).collect();
    let items: Vec<(String, String, String, i64)> = p
        .items
        .iter()
        .map(|it| {
            (
                match (it.start, it.end) {
                    (Some(s), Some(e)) => format!("{}–{}", s.format("%H:%M"), e.format("%H:%M")),
                    _ => String::new(),
                },
                it.task_name.clone(),
                if it.duration_min > 0 {
                    format!("{} 分", it.duration_min)
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

            <div class="plan-list">
                {items.iter().map(|(time, name, dur, task_id)| {
                    let task_id = *task_id;
                    let time_clone = time.clone();
                    let name_clone = name.clone();
                    let dur_clone = dur.clone();
                    let is_pending = move || pending_ids.get().contains(&task_id);
                    view! {
                        <div class="plan-item" class:pending=is_pending>
                            <div class="plan-time">
                                {if time_clone.is_empty() { "随时".to_string() } else { time_clone.clone() }}
                            </div>
                            <div class="plan-name">{name_clone.clone()}</div>
                            {(!dur_clone.is_empty()).then(|| view! { <div class="plan-dur">{dur_clone.clone()}</div> })}
                            <div class="plan-check">
                                <input type="checkbox" />
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
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
            <p class="item-count">"共 " {items_len} " 项"</p>

            {move || {
                let pending = pending_ids.get();
                if pending.is_empty() { return view! { <span></span> }.into_any(); }
                let pending_items: Vec<&(String, String, String, i64)> = items.iter()
                    .filter(|(_, _, _, id)| pending.contains(id))
                    .collect();
                view! {
                    <div class="pending-section">
                        <h4>"待定事项"</h4>
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
