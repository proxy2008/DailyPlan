mod app;
mod calendar;
mod day_view;
mod sidebar;
mod tauri;
mod task_editor;
mod task_list;
mod task_manage;

use app::*;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! {
            <App/>
        }
    })
}
