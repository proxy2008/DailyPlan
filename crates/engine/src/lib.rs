//! DailyPlan 调度引擎：从任务库 + 日期 → 当日打卡表。
//!
//! 纯逻辑，无 IO。负责：
//! - 把命中当天的 Task 展开成 ChecklistItem（按 slot 数展开）
//! - 按 start 时间排序
//! - 检测时段重叠，产出 Conflict 告警

pub mod conflict;
pub mod render;
pub mod scheduler;

pub use scheduler::build_day_plan;
