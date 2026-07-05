mod app;
mod calendar;
mod day_view;
mod tauri;
mod task_editor;
mod task_list;

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
