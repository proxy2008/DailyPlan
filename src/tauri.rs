//! Tauri 命令调用封装。
//!
//! 通过全局 window.__TAURI__.core.invoke 调后端命令。
//! 这是 Tauri 2 + wasm 的官方推荐模式（见 withGlobalTauri 配置）。

use dailyplan_domain::{DayPlan, Task};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

/// 列出全部任务。
pub async fn list_tasks() -> Result<Vec<Task>, String> {
    let raw = invoke("list_tasks", JsValue::UNDEFINED).await;
    serde_wasm_bindgen::from_value::<Vec<Task>>(raw).map_err(|e| e.to_string())
}

/// 新建任务（返回带新 id 的副本）。
pub async fn create_task(task: Task) -> Result<Task, String> {
    // Tauri 命令按参数名取值：后端 fn create_task(task: Task) 要求 { task: {...} }。
    #[derive(Serialize)]
    struct Args {
        task: Task,
    }
    let args = serde_wasm_bindgen::to_value(&Args { task }).map_err(|e| e.to_string())?;
    let raw = invoke("create_task", args).await;
    serde_wasm_bindgen::from_value::<Task>(raw).map_err(|e| e.to_string())
}

/// 更新任务。
pub async fn update_task(task: Task) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        task: Task,
    }
    let args = serde_wasm_bindgen::to_value(&Args { task }).map_err(|e| e.to_string())?;
    let _ = invoke("update_task", args).await;
    Ok(())
}

/// 删除任务。
pub async fn delete_task(task_id: i64) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        task_id: i64,
    }
    let args = serde_wasm_bindgen::to_value(&Args { task_id }).map_err(|e| e.to_string())?;
    let _ = invoke("delete_task", args).await;
    Ok(())
}

/// 生成某天的打卡表。date 格式 YYYY-MM-DD。
pub async fn generate_day(date: &str) -> Result<DayPlan, String> {
    #[derive(Serialize)]
    struct Args<'a> {
        date: &'a str,
    }
    let args = serde_wasm_bindgen::to_value(&Args { date }).map_err(|e| e.to_string())?;
    let raw = invoke("generate_day", args).await;
    serde_wasm_bindgen::from_value::<DayPlan>(raw).map_err(|e| e.to_string())
}

/// 前端传给 print_day 的单个 item（镜像后端 PrintItemInput）。
/// pending 由 DayView 的 pending_ids 信号填充。
#[derive(Serialize, Clone)]
pub struct PrintItemInput {
    pub time: Option<String>,
    pub task_name: String,
    pub duration_min: u32,
    pub pending: bool,
}

/// 打印某天（生成 PDF 并用系统查看器打开）。返回 PDF 路径。
///
/// 后端契约：`print_day(app, date: String, items: Vec<PrintItemInput>)`，
/// 前端把已标记 pending 的 items 连同选定日期一起传入。
pub async fn print_day(date: &str, items: Vec<PrintItemInput>) -> Result<String, String> {
    #[derive(Serialize)]
    struct Args<'a> {
        date: &'a str,
        items: Vec<PrintItemInput>,
    }
    let args = serde_wasm_bindgen::to_value(&Args { date, items }).map_err(|e| e.to_string())?;
    let raw = invoke("print_day", args).await;
    serde_wasm_bindgen::from_value::<String>(raw).map_err(|e| e.to_string())
}

// 为前端编辑表单方便，重新导出 domain 类型
#[allow(unused_imports)]
pub use dailyplan_domain::{Frequency, TimeSlot};
