//! 应用根组件：任务列表 + 当日打卡表 + 编辑器切换。

use dailyplan_domain::Task;
use leptos::prelude::*;
use leptos::task::spawn_local;

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

    let start_create = move || {
        editor_state.set(EditorState::default());
        panel.set(Panel::Editor);
    };

    let start_edit = move |state: EditorState| {
        editor_state.set(state);
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
    // 后端签名（Task 6 后）需同时传 date + items。
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
        <header class="app-header">
            <h1>"📅 每日计划表"</h1>
            <p class="subtitle">"录入任务 · 绑定时段 · 生成打卡表 · 打印检查"</p>
        </header>

        <main class="app-main">
            <section class="col col-left">
                <DayView date on_print={move |d: String, items: Vec<crate::tauri::PrintItemInput>| on_print(d, items)} />
            </section>

            <section class="col col-right">
                {move || {
                    if panel.get() == Panel::Editor {
                        let state = editor_state.get();
                        view! {
                            <TaskEditor
                                initial={state}
                                on_saved={on_saved}
                                on_cancel={on_cancel}
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="list-panel">
                                <button class="primary block" on:click=move |_| start_create()>"+ 新建任务"</button>
                                <TaskList tasks on_edit={start_edit} on_refresh={move || refresh.with_value(|f| f())} />
                            </div>
                        }.into_any()
                    }
                }}
            </section>
        </main>
    }.into_any()
}
