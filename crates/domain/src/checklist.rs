//! 每日打卡表的结构：调度结果 + 冲突告警。

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

/// 一天里需要打卡的一条具体事项（由调度引擎从 Task 展开而来）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub task_id: i64,
    pub task_name: String,
    pub start: NaiveTime,
    pub end: NaiveTime,
    /// 时长（分钟），便于打印。
    pub duration_min: u32,
}

/// 两个事项发生时段重叠的告警。MVP 只告警，不自动改时段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// 参与冲突的两个事项（按 start 排序后存索引）。
    pub item_a: usize,
    pub item_b: usize,
    pub message: String,
}

/// 某一天的完整计划表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayPlan {
    pub date: NaiveDate,
    pub items: Vec<ChecklistItem>,
    pub conflicts: Vec<Conflict>,
}

impl DayPlan {
    pub fn empty(date: NaiveDate) -> Self {
        Self {
            date,
            items: Vec::new(),
            conflicts: Vec::new(),
        }
    }
}
