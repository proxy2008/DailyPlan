//! Tauri 命令调用封装。
//!
//! 通过 window.safeInvoke（index.html 里定义）调 Tauri 后端命令。
//! safeInvoke catch invoke 的 reject，返回 {ok, val} 或 {ok:false, err}，
//! 避免 wasm-bindgen async extern 在后端报错时 panic。

use dailyplan_domain::{DayPlan, Task};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    // index.html 里定义的 window.safeInvoke，catch reject 返回 {ok, val/err}。
    #[wasm_bindgen(js_name = "safeInvoke")]
    async fn safe_invoke(cmd: String, args: JsValue) -> JsValue;
}

/// 调后端命令，reject 时返回 Err(错误字符串) 而非 panic。
async fn invoke_safe(cmd: &str, args: JsValue) -> Result<JsValue, String> {
    let raw = safe_invoke(cmd.to_string(), args).await;
    let ok = js_sys::Reflect::get(&raw, &"ok".into()).unwrap_or(JsValue::FALSE);
    if ok.is_truthy() {
        Ok(js_sys::Reflect::get(&raw, &"val".into()).unwrap_or(JsValue::UNDEFINED))
    } else {
        let err = js_sys::Reflect::get(&raw, &"err".into()).unwrap_or(JsValue::UNDEFINED);
        Err(err.as_string().unwrap_or_else(|| "unknown error".into()))
    }
}

/// 列出全部任务。
pub async fn list_tasks() -> Result<Vec<Task>, String> {
    let raw = invoke_safe("list_tasks", JsValue::UNDEFINED).await?;
    serde_wasm_bindgen::from_value::<Vec<Task>>(raw).map_err(|e| e.to_string())
}

/// 新建任务（返回带新 id 的副本）。
pub async fn create_task(task: Task) -> Result<Task, String> {
    #[derive(Serialize)]
    struct Args {
        task: Task,
    }
    let args = serde_wasm_bindgen::to_value(&Args { task }).map_err(|e| e.to_string())?;
    let raw = invoke_safe("create_task", args).await?;
    serde_wasm_bindgen::from_value::<Task>(raw).map_err(|e| e.to_string())
}

/// 更新任务。
pub async fn update_task(task: Task) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        task: Task,
    }
    let args = serde_wasm_bindgen::to_value(&Args { task }).map_err(|e| e.to_string())?;
    invoke_safe("update_task", args).await?;
    Ok(())
}

/// 删除任务。
pub async fn delete_task(task_id: i64) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        task_id: i64,
    }
    let args = serde_wasm_bindgen::to_value(&Args { task_id }).map_err(|e| e.to_string())?;
    invoke_safe("delete_task", args).await?;
    Ok(())
}

/// 生成某天的打卡表。date 格式 YYYY-MM-DD。
pub async fn generate_day(date: &str) -> Result<DayPlan, String> {
    #[derive(Serialize)]
    struct Args<'a> {
        date: &'a str,
    }
    let args = serde_wasm_bindgen::to_value(&Args { date }).map_err(|e| e.to_string())?;
    let raw = invoke_safe("generate_day", args).await?;
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
pub async fn print_day(date: &str, items: Vec<PrintItemInput>) -> Result<String, String> {
    #[derive(Serialize)]
    struct Args<'a> {
        date: &'a str,
        items: Vec<PrintItemInput>,
    }
    let args = serde_wasm_bindgen::to_value(&Args { date, items }).map_err(|e| e.to_string())?;
    let raw = invoke_safe("print_day", args).await?;
    serde_wasm_bindgen::from_value::<String>(raw).map_err(|e| e.to_string())
}

// 为前端编辑表单方便，重新导出 domain 类型
#[allow(unused_imports)]
pub use dailyplan_domain::{Frequency, TimeSlot};
