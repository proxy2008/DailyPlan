//! 应用根组件：任务列表 + 当日打卡表 + 编辑器切换。

use dailyplan_domain::Task;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::day_view::DayView;
use crate::sidebar::{Page, Sidebar};
use crate::task_editor::{EditorState, TaskEditor};
use crate::task_manage::TaskManage;

/// 当前是否在编辑器浮层（modal）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    List,
    Editor,
}

/// 应用根组件。
#[component]
pub fn App() -> impl IntoView {
    // 当前页面（日程 / 任务管理）
    let page = RwSignal::new(Page::Schedule);
    // 编辑器浮层开关
    let panel = RwSignal::new(Panel::List);
    // 编辑器初始状态
    let editor_state = RwSignal::new(EditorState::default());
    // 任务列表数据
    let (tasks, set_tasks) = signal::<Vec<Task>>(Vec::new());
    // tasks 修订号：每次 refresh 递增，强制 TaskList 的 <For> 重建（编辑后内容变了但 id 没变）
    let tasks_rev = RwSignal::new(0u32);
    // 删除确认状态：Some(id) 表示正在确认删除该任务
    let confirming: RwSignal<Option<i64>> = RwSignal::new(None);
    // 当前查看的日期
    let date = RwSignal::new(chrono::Local::now().date_naive().format("%Y-%m-%d").to_string());

    // 加载任务列表
    let refresh = StoredValue::new(move || {
        spawn_local(async move {
            web_sys::console::log_1(&"[refresh] 开始 list_tasks".into());
            match crate::tauri::list_tasks().await {
                Ok(list) => {
                    set_tasks.set(list);
                    tasks_rev.update(|v| *v = v.wrapping_add(1));
                }
                Err(e) => web_sys::console::error_1(&format!("加载任务失败: {e}").into()),
            }
        });
    });

    // 初始加载
    refresh.with_value(|f| f());

    // 全局事件委托：处理 For 内动态渲染的编辑/删除按钮。
    // Leptos 0.8 的 on:click 在 For 子项里不稳定，改用 document 级 click 监听 + data 属性。
    {
        let tasks_sig = tasks;
        let editor_sig = editor_state;
        let panel_sig = panel;
        let refresh_fn = refresh;
        let confirming_sig = confirming;
        spawn_local(async move {
            let Some(win) = web_sys::window() else { return };
            let Some(doc) = win.document() else { return };
            let handler = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::Event)>::new(move |ev: web_sys::Event| {
                let Some(target) = ev.target() else { return };
                let el: web_sys::Element = target.unchecked_into();
                let btn = el.closest("[data-action]").ok().flatten();
                let Some(btn) = btn else { return };
                let action = btn.get_attribute("data-action").unwrap_or_default();
                let id_str = btn.get_attribute("data-task-id").unwrap_or_default();
                let id: i64 = id_str.parse().unwrap_or(-1);
                if id < 0 { return; }
                match action.as_str() {
                    "edit" => {
                        let list = tasks_sig.get();
                        if let Some(t) = list.iter().find(|t| t.id == id) {
                            let state = EditorState::from_task(t);
                            editor_sig.set(state);
                            panel_sig.set(Panel::Editor);
                        }
                    }
                    "delete" => {
                        // 第一次点：显示确认
                        confirming_sig.set(Some(id));
                    }
                    "confirm-delete" => {
                        // 点"是"：真删
                        confirming_sig.set(None);
                        let refresh_fn = refresh_fn;
                        spawn_local(async move {
                            match crate::tauri::delete_task(id).await {
                                Ok(()) => refresh_fn.with_value(|f| f()),
                                Err(e) => {
                                    web_sys::console::error_1(&format!("删除失败: {e}").into());
                                }
                            }
                        });
                    }
                    "cancel-delete" => {
                        // 点"否"：取消
                        confirming_sig.set(None);
                    }
                    _ => {}
                }
            });
            let _ = doc.add_event_listener_with_callback("click", handler.as_ref().unchecked_ref());
            std::mem::forget(handler);
        });
    }

    let start_create = move || {
        editor_state.set(EditorState::default());
        panel.set(Panel::Editor);
    };

    let on_saved = move || {
        web_sys::console::log_1(&"[on_saved] 保存回调触发，关闭 modal + refresh".into());
        panel.set(Panel::List);
        refresh.with_value(|f| f());
    };

    let on_cancel = move || {
        panel.set(Panel::List);
    };

    // 打印：调后端 print_day 生成 PDF 并打开。
    // 把用户选定日期 + 已标记 pending 的 items 一起传入。
    let on_print = move |date_str: String, items: Vec<crate::tauri::PrintItemInput>| {
        spawn_local(async move {
            match crate::tauri::print_day(&date_str, items).await {
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

    // 多日打印弹窗状态
    let multi_day_open = RwSignal::new(false);
    let multi_day_count = RwSignal::new(7u32);
    let multi_day_busy = RwSignal::new(false);

    // 触发多日弹窗（从 DayView 的打印下拉菜单调用）
    let on_print_days = move || {
        multi_day_count.set(7);
        multi_day_open.set(true);
    };

    // 执行多日打印：从当前选中日期起 N 天
    let do_multi_print = move || {
        let start = date.get();
        let days = multi_day_count.get().clamp(1, 31);
        multi_day_busy.set(true);
        spawn_local(async move {
            match crate::tauri::print_days(&start, days).await {
                Ok(_path) => {
                    multi_day_busy.set(false);
                    multi_day_open.set(false);
                    if let Some(w) = web_sys::window() {
                        let _ = w.alert_with_message(&format!(
                            "已生成 {} 天的 PDF 并打开（从 {} 起）",
                            days, start
                        ));
                    }
                }
                Err(e) => {
                    multi_day_busy.set(false);
                    if let Some(w) = web_sys::window() {
                        let _ = w.alert_with_message(&format!("多日打印失败: {e}"));
                    }
                }
            }
        });
    };

    view! {
        <div class="app-shell">
            <Sidebar page />
            <main class="page">
                {move || match page.get() {
                    Page::Schedule => view! {
                        <section class="page-schedule">
                            <DayView
                                date
                                tasks
                                on_print={move |d: String, items: Vec<crate::tauri::PrintItemInput>| on_print(d, items)}
                                on_print_days={move || on_print_days()}
                            />
                        </section>
                    }.into_any(),
                    Page::TaskManage => view! {
                        <section class="page-tasks">
                            <TaskManage
                                tasks
                                confirming
                                tasks_rev
                                on_create={move || start_create()}
                            />
                        </section>
                    }.into_any(),
                }}
            </main>
        </div>

        // 编辑器浮层（modal）：点新建/编辑时弹出，不改变底层布局
        {move || {
            if panel.get() == Panel::Editor {
                let state = editor_state.get();
                Some(view! {
                    <div class="modal-overlay" on:click=move |_| on_cancel()>
                        <div class="modal-content" on:click=move |ev| ev.stop_propagation()>
                            <TaskEditor
                                initial={state}
                                on_saved={on_saved}
                                on_cancel={on_cancel}
                            />
                        </div>
                    </div>
                })
            } else {
                None
            }
        }}

        // 多日打印弹窗
        {move || {
            if !multi_day_open.get() {
                return None;
            }
            let today_str = date.get();
            Some(view! {
                <div class="modal-overlay" on:click=move |_| multi_day_open.set(false)>
                    <div class="modal-content multi-day-dialog" on:click=move |ev| ev.stop_propagation()>
                        <h3>"打印多日"</h3>
                        <p>"从 " {today_str.clone()} " 起，连续 "
                            <input type="number" class="multi-day-input" min="1" max="31"
                                prop:value=move || multi_day_count.get()
                                on:input=move |ev| {
                                    let v = event_target_value(&ev).parse::<u32>().unwrap_or(7);
                                    multi_day_count.set(v);
                                } />
                            " 天"
                        </p>
                        <p class="multi-day-hint">"将生成多天的打卡表，每天一页，合并到 1 个 PDF。"</p>
                        <div class="editor-actions">
                            <button on:click=move |_| multi_day_open.set(false)>"取消"</button>
                            <button class="primary"
                                disabled=move || multi_day_busy.get()
                                on:click=move |_| do_multi_print()>
                                {move || if multi_day_busy.get() { "生成中…" } else { "生成 PDF" }}
                            </button>
                        </div>
                    </div>
                </div>
            })
        }}
    }.into_any()
}
