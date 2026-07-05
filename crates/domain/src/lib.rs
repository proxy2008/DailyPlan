//! DailyPlan 领域模型（domain）。
//!
//! 这个 crate 只定义数据结构和纯逻辑，不依赖任何 IO（无数据库、无网络、无文件）。
//! 前端（Leptos/wasm）和后端（Tauri/Rust）共享这些类型，保证命令契约一致。

pub mod checklist;
pub mod task;

pub use checklist::{ChecklistItem, Conflict, DayPlan};
pub use task::{Frequency, Task, TimeSlot};
