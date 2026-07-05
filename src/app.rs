//! 应用根组件：任务列表 + 当日打卡表 + 编辑器切换。

use dailyplan_domain::Task;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::day_view::DayView;
use crate::task_editor::{EditorState, TaskEditor};
use crate::task_list::TaskList;

/// 当前展示哪个面板。
#[derive(Clone, PartialEq)]
enum Panel {
    List,
    Editor,
}

/// 应用根组件。
#[component]
pub fn App() -> impl IntoView {
    // 视图切换
    let panel = RwSignal::new(Panel::List);
    // 编辑器初始状态
    let editor_state = RwSignal::new(EditorState::default());
    // 任务列表数据
    let (tasks, set_tasks) = signal::<Vec<Task>>(Vec::new());
    // 删除确认状态：Some(id) 表示正在确认删除该任务
    let confirming: RwSignal<Option<i64>> = RwSignal::new(None);
    // 当前查看的日期
    let date = RwSignal::new(chrono::Local::now().date_naive().format("%Y-%m-%d").to_string());

    // 加载任务列表
    let refresh = StoredValue::new(move || {
        spawn_local(async move {
            match crate::tauri::list_tasks().await {
                Ok(list) => set_tasks.set(list),
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

    view! {
        <main class="app-main">
            <section class="col col-left">
                <DayView date tasks on_print={move |d: String, items: Vec<crate::tauri::PrintItemInput>| on_print(d, items)} />
            </section>

            <section class="col col-right">
                <div class="list-panel">
                    <button class="primary block" on:click=move |_| start_create()>"+ 新建任务"</button>
                    <TaskList tasks confirming />
                </div>
            </section>
        </main>

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
    }.into_any()
}
