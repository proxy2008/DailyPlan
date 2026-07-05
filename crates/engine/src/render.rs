//! 打印渲染的数据准备。
//!
//! engine 不直接调 Typst（那需要进程 IO，放 src-tauri 里）。
//! 这里只负责：
//! - 把 DayPlan + 元信息序列化成 Typst 模板要的 JSON 结构
//! - 提供嵌入的 checklist.typ 模板字符串
//!
//! src-tauri 的 print_day 命令会把这些写到 temp 目录，再 spawn typst。

use chrono::Datelike;
use dailyplan_domain::DayPlan;
use serde::Serialize;

/// 传给 Typst 模板的、与当天计划无关的渲染选项。
#[derive(Debug, Clone, Serialize)]
pub struct RenderOptions {
    /// 标题，如 "每日计划表"。
    pub title: String,
    /// 复盘区是否打印。
    pub with_review: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            title: "每日计划表".into(),
            with_review: true,
        }
    }
}

/// 给 Typst 用的、合并好的打印数据（对应 data.json）。
#[derive(Debug, Serialize)]
pub struct PrintData {
    pub title: String,
    pub date: String, // YYYY-MM-DD
    pub weekday_cn: String,
    pub items: Vec<PrintItem>,
    pub conflicts: Vec<String>,
    pub with_review: bool,
}

#[derive(Debug, Serialize)]
pub struct PrintItem {
    pub time: String,      // "06:30-07:00"
    pub task_name: String,
    pub duration_min: u32,
    pub note: String, // 空字符串，留手写
}

/// 把 DayPlan + 选项转成可序列化的 PrintData。
pub fn to_print_data(plan: &DayPlan, opts: &RenderOptions) -> PrintData {
    let weekday_cn = match plan.date.weekday() {
        chrono::Weekday::Mon => "周一",
        chrono::Weekday::Tue => "周二",
        chrono::Weekday::Wed => "周三",
        chrono::Weekday::Thu => "周四",
        chrono::Weekday::Fri => "周五",
        chrono::Weekday::Sat => "周六",
        chrono::Weekday::Sun => "周日",
    };
    PrintData {
        title: opts.title.clone(),
        date: plan.date.format("%Y-%m-%d").to_string(),
        weekday_cn: weekday_cn.to_string(),
        items: plan
            .items
            .iter()
            .map(|it| PrintItem {
                time: format!(
                    "{}-{}",
                    it.start.format("%H:%M"),
                    it.end.format("%H:%M")
                ),
                task_name: it.task_name.clone(),
                duration_min: it.duration_min,
                note: String::new(),
            })
            .collect(),
        conflicts: plan.conflicts.iter().map(|c| c.message.clone()).collect(),
        with_review: opts.with_review,
    }
}

/// 嵌入的 Typst 模板（见 templates/checklist.typ）。
pub const CHECKLIST_TYP: &str = include_str!("../templates/checklist.typ");

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use dailyplan_domain::checklist::DayPlan;

    #[test]
    fn print_data_basic() {
        let plan = DayPlan::empty(NaiveDate::from_ymd_opt(2026, 7, 4).unwrap());
        let data = to_print_data(&plan, &RenderOptions::default());
        assert_eq!(data.date, "2026-07-04");
        assert_eq!(data.weekday_cn, "周六");
        assert!(data.items.is_empty());
    }

    #[test]
    fn template_embedded() {
        // 模板非空且包含 set text 指令
        assert!(CHECKLIST_TYP.contains("#set text") || CHECKLIST_TYP.contains("#set page"));
    }
}
