//! Tauri 应用入口：注册状态（DB）、迁移、命令。

mod commands;
mod db;
mod printing;

use std::sync::Mutex;
use tauri::Manager;

use db::Db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // DB 文件放 app_data_dir（macOS: ~/Library/Application Support/com.dailyplan.app/）
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("dailyplan.db");
            let conn = db::open_and_migrate(&db_path)?;
            app.manage(Db(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::delete_task,
            commands::generate_day,
            commands::print_day,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
