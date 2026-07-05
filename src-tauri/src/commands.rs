//! Tauri 命令：前后端契约的实现层。
//!
//! 前端（Leptos/wasm）通过 invoke("list_tasks") 等调用这里。
//! 命令签名要与 frontend/src/tauri.rs 里的调用一一对应。

use chrono::NaiveDate;
use serde::Serialize;
use dailyplan_domain::{DayPlan, Task};
use dailyplan_engine::build_day_plan;
use tauri::State;

use crate::db::{self, Db};

// ===== 错误类型：Tauri 命令的 Result 必须能序列化给前端 =====

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(String),
    #[error("日期格式错误，需 YYYY-MM-DD: {0}")]
    BadDate(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

type AppResult<T> = Result<T, AppError>;

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Db(e.to_string())
}

// ===== 任务 CRUD =====

#[tauri::command]
pub fn list_tasks(db: State<'_, Db>) -> AppResult<Vec<Task>> {
    db::list_tasks(&db).map_err(db_err)
}

#[tauri::command]
pub fn create_task(db: State<'_, Db>, task: Task) -> AppResult<Task> {
    db::insert_task(&db, &task).map_err(db_err)
}

#[tauri::command]
pub fn update_task(db: State<'_, Db>, task: Task) -> AppResult<()> {
    db::update_task(&db, &task).map_err(db_err)
}

#[tauri::command]
pub fn delete_task(db: State<'_, Db>, task_id: i64) -> AppResult<()> {
    db::delete_task(&db, task_id).map_err(db_err)
}

// ===== 当日计划 =====

#[tauri::command]
pub fn generate_day(db: State<'_, Db>, date: String) -> AppResult<DayPlan> {
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| AppError::BadDate(e.to_string()))?;
    let tasks = db::list_tasks(&db).map_err(db_err)?;
    Ok(build_day_plan(date, &tasks))
}

/// 打印某天：生成 PDF 并用系统查看器打开。
#[tauri::command]
pub async fn print_day(
    app: tauri::AppHandle,
    date: String,
    items: Vec<dailyplan_engine::render::PrintItemInput>,
) -> AppResult<String> {
    let pdf_path = crate::printing::print_day(&app, &date, items).await?;
    Ok(pdf_path.to_string_lossy().to_string())
}
