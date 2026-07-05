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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // dev 模式：数据放项目下的 .dev-data/，与正式安装版隔离
            // release 模式：放系统 app_data_dir
            #[cfg(debug_assertions)]
            let dir = {
                let dev_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".dev-data");
                std::fs::create_dir_all(&dev_dir)?;
                dev_dir
            };
            #[cfg(not(debug_assertions))]
            let dir = {
                let d = app.path().app_data_dir()?;
                std::fs::create_dir_all(&d)?;
                d
            };
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
