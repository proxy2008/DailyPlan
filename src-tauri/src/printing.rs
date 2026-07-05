//! 打印：调用 Typst sidecar 生成 PDF 并用系统查看器打开。
//!
//! 流程：
//! 1. 用 engine 把 DayPlan 转成 PrintData（JSON）+ 拿到 checklist.typ 模板字符串
//! 2. 写到 temp 目录（data.json + checklist.typ）
//! 3. 用 tauri-plugin-shell 的 sidecar 调 `typst compile checklist.typ out.pdf`
//! 4. 用 opener 插件打开生成的 PDF（macOS 走 Preview）

use std::path::PathBuf;

use chrono::NaiveDate;
use dailyplan_engine::render::{to_print_data, RenderOptions};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::commands::AppError;
use crate::db::Db;

/// 为指定日期生成 PDF 并打开。
pub async fn print_day(app: &AppHandle, date_str: &str) -> Result<PathBuf, AppError> {
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| AppError::BadDate(e.to_string()))?;

    // 读 DB（同步，await 前）
    let db_state = app.state::<Db>();
    let tasks = crate::db::list_tasks(&db_state).map_err(|e| AppError::Db(e.to_string()))?;
    let plan = dailyplan_engine::build_day_plan(date, &tasks);

    let print_data = to_print_data(&plan, &RenderOptions::default());
    let template = dailyplan_engine::render::CHECKLIST_TYP;

    // 写到 app_data_dir/print/ 下（持久、用户可找）
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Db(format!("路径解析失败: {e}")))?;
    let print_dir = dir.join("print");
    std::fs::create_dir_all(&print_dir)
        .map_err(|e| AppError::Db(format!("创建目录失败: {e}")))?;

    let typ_path = print_dir.join("checklist.typ");
    let data_path = print_dir.join("data.json");
    let pdf_path = print_dir
        .join(format!("dailyplan-{}.pdf", date.format("%Y-%m-%d")));

    let data_json = serde_json::to_string_pretty(&print_data)
        .map_err(|e| AppError::Db(format!("序列化失败: {e}")))?;
    std::fs::write(&typ_path, template)
        .map_err(|e| AppError::Db(format!("写模板失败: {e}")))?;
    std::fs::write(&data_path, data_json)
        .map_err(|e| AppError::Db(format!("写数据失败: {e}")))?;

    // 调 typst sidecar 编译。
    // --font-path 指向系统字体目录，确保找到 PingFang SC 等中文字体。
    let shell = app.shell();
    let cmd = shell
        .sidecar("typst")
        .map_err(|e| AppError::Db(format!("启动 typst 失败: {e}")))?;
    let (mut rx, _child) = cmd
        .args([
            "compile",
            "--font-path",
            "/System/Library/Fonts:/Library/Fonts:~/Library/Fonts",
            typ_path.to_str().unwrap(),
            pdf_path.to_str().unwrap(),
        ])
        .spawn()
        .map_err(|e| AppError::Db(format!("spawn typst 失败: {e}")))?;

    // 等 typst 退出
    let mut stderr_output = String::new();
    let mut exit_code: Option<i32> = None;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stderr(line) => {
                stderr_output.push_str(&String::from_utf8_lossy(&line));
                stderr_output.push('\n');
            }
            CommandEvent::Terminated(status) => {
                exit_code = status.code;
            }
            _ => {}
        }
    }

    // 检查退出码：None 或非 0 视为失败
    if let Some(code) = exit_code {
        if code != 0 {
            return Err(AppError::Db(format!(
                "typst 编译失败 (code {code}): {stderr_output}"
            )));
        }
    }

    // 用 opener 插件打开 PDF（macOS 走 Preview）
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(pdf_path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| AppError::Db(format!("打开 PDF 失败: {e}")))?;

    Ok(pdf_path)
}
