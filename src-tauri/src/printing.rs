//! 打印：调用 Typst sidecar 生成 PDF 并用系统查看器打开。
//!
//! 流程：
//! 1. 用 engine 把 DayPlan 转成 PrintData（JSON）+ 拿到 checklist.typ 模板字符串
//! 2. 写到 temp 目录（data.json + checklist.typ）
//! 3. 用 tauri-plugin-shell 的 sidecar 调 `typst compile checklist.typ out.pdf`
//! 4. 用 opener 插件打开生成的 PDF（macOS 走 Preview）

use std::path::PathBuf;

use dailyplan_engine::render::PrintItemInput;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::commands::AppError;

/// 为指定日期生成 PDF 并打开。
pub async fn print_day(
    app: &AppHandle,
    date_str: &str,
    items: Vec<PrintItemInput>,
) -> Result<PathBuf, AppError> {
    let print_data = dailyplan_engine::render::to_print_data_from_items(
        items,
        date_str,
        &dailyplan_engine::render::RenderOptions::default(),
    );
    let template = dailyplan_engine::render::CHECKLIST_TYP;

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Db(format!("路径解析失败: {e}")))?;
    let print_dir = dir.join("print");
    std::fs::create_dir_all(&print_dir)
        .map_err(|e| AppError::Db(format!("创建目录失败: {e}")))?;

    let typ_path = print_dir.join("checklist.typ");
    let data_path = print_dir.join("data.json");
    let pdf_path = print_dir.join(format!("dailyplan-{date_str}.pdf"));

    let data_json = serde_json::to_string_pretty(&print_data)
        .map_err(|e| AppError::Db(format!("序列化失败: {e}")))?;
    std::fs::write(&typ_path, template)
        .map_err(|e| AppError::Db(format!("写模板失败: {e}")))?;
    std::fs::write(&data_path, data_json)
        .map_err(|e| AppError::Db(format!("写数据失败: {e}")))?;

    let shell = app.shell();
    let cmd = shell
        .sidecar("typst")
        .map_err(|e| AppError::Db(format!("启动 typst 失败: {e}")))?;
    let (mut rx, _child) = cmd
        .args([
            "compile",
            "--font-path",
            // 跨平台字体目录：macOS / Windows / Linux
            if cfg!(target_os = "macos") {
                "/System/Library/Fonts:/Library/Fonts:~/Library/Fonts"
            } else if cfg!(target_os = "windows") {
                "C:\\Windows\\Fonts"
            } else {
                "/usr/share/fonts:/usr/local/share/fonts:~/.fonts"
            },
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
