//! 调度：给定日期 + 任务列表 → DayPlan。

use chrono::NaiveDate;
use dailyplan_domain::{
    checklist::{ChecklistItem, DayPlan},
    task::Task,
};

use crate::conflict::detect_conflicts;

/// 为 `date` 这一天，从 `tasks` 里生成完整的打卡表。
///
/// 步骤：
/// 1. 过滤出 active 且频率命中当天的任务。
/// 2. 把每个任务的 slots 展开成 ChecklistItem（Daily 频率下用全部 slot；
///    其它频率下也用全部 slot——MVP 不做 slot 配额）。
/// 3. 按 start 排序。
/// 4. 检测冲突。
pub fn build_day_plan(date: NaiveDate, tasks: &[Task]) -> DayPlan {
    let mut items: Vec<ChecklistItem> = tasks
        .iter()
        .filter(|t| t.active && t.frequency.matches(date))
        .flat_map(|t| {
            let req = t.description.clone().unwrap_or_default();
            if t.slots.is_empty() {
                // 无时段任务：产出单个 untimed item
                vec![ChecklistItem {
                    task_id: t.id,
                    task_name: t.name.clone(),
                    start: None,
                    end: None,
                    duration_min: 0,
                    priority: t.priority_level,
                    pending: false,
                    requirement: req,
                }]
            } else {
                t.slots
                    .iter()
                    .map(move |slot| ChecklistItem {
                        task_id: t.id,
                        task_name: t.name.clone(),
                        start: Some(slot.start),
                        end: Some(slot.end),
                        duration_min: slot.duration_minutes(),
                        priority: t.priority_level,
                        pending: false,
                        requirement: req.clone(),
                    })
                    .collect::<Vec<_>>()
            }
        })
        .collect();

    // 排序键：无时段 (start=None) 排最后；定时按 start 升序；
    // 同 start 按优先级降序；最后 task_id 升序稳定 tiebreak。
    items.sort_by(|a, b| {
        a.start.is_none()
            .cmp(&b.start.is_none())
            .then_with(|| a.start.cmp(&b.start))
            .then_with(|| b.priority.rank().cmp(&a.priority.rank()))
            .then_with(|| a.task_id.cmp(&b.task_id))
    });

    let conflicts = detect_conflicts(&items);

    DayPlan {
        date,
        items,
        conflicts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use dailyplan_domain::task::{Frequency, PriorityLevel, TimeSlot};

    fn slot(s: &str, e: &str) -> TimeSlot {
        TimeSlot {
            start: NaiveTime::parse_from_str(s, "%H:%M").unwrap(),
            end: NaiveTime::parse_from_str(e, "%H:%M").unwrap(),
        }
    }

    fn task(id: i64, name: &str, freq: Frequency, slots: Vec<TimeSlot>) -> Task {
        Task {
            id,
            name: name.into(),
            description: None,
            frequency: freq,
            slots,
            priority_level: PriorityLevel::Normal,
            active: true,
        }
    }

    #[test]
    fn empty_tasks_gives_empty_plan() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let plan = build_day_plan(date, &[]);
        assert!(plan.items.is_empty());
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn skips_inactive_and_non_matching() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap(); // 周六
        let active_match = task(
            1,
            "晨跑",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("06:30", "07:00")],
        );
        let mut inactive = task(
            2,
            "读书",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("08:00", "09:00")],
        );
        inactive.active = false;
        let not_today = task(
            3,
            "体检",
            Frequency::Once {
                date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            },
            vec![slot("09:00", "11:00")],
        );

        let plan = build_day_plan(date, &[active_match, inactive, not_today]);
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].task_name, "晨跑");
    }

    #[test]
    fn expands_multiple_slots_of_one_task() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let drink_water = task(
            1,
            "喝水",
            Frequency::Daily { times_per_day: 3 },
            vec![
                slot("08:00", "08:05"),
                slot("12:00", "12:05"),
                slot("18:00", "18:05"),
            ],
        );
        let plan = build_day_plan(date, &[drink_water]);
        assert_eq!(plan.items.len(), 3);
        // 排序后按 start 升序
        assert_eq!(
            plan.items[0].start.unwrap().format("%H:%M").to_string(),
            "08:00"
        );
        assert_eq!(
            plan.items[2].start.unwrap().format("%H:%M").to_string(),
            "18:00"
        );
    }

    #[test]
    fn detects_overlap_conflict() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let a = task(
            1,
            "A",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("07:00", "08:00")],
        );
        let b = task(
            2,
            "B",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("07:30", "08:30")],
        );
        let plan = build_day_plan(date, &[a, b]);
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.conflicts.len(), 1, "应有 1 个冲突告警");
    }

    #[test]
    fn no_conflict_when_back_to_back() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let a = task(
            1,
            "A",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("07:00", "08:00")],
        );
        let b = task(
            2,
            "B",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("08:00", "09:00")],
        );
        let plan = build_day_plan(date, &[a, b]);
        assert!(plan.conflicts.is_empty(), "首尾相接不算冲突");
    }

    #[test]
    fn untimed_task_produces_one_item_at_end() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let timed = task(
            1,
            "晨跑",
            Frequency::Daily { times_per_day: 1 },
            vec![slot("06:30", "07:00")],
        );
        let untimed = task(
            2,
            "读书",
            Frequency::Daily { times_per_day: 1 },
            vec![], // 无 slots
        );
        let plan = build_day_plan(date, &[untimed, timed]);
        assert_eq!(plan.items.len(), 2);
        // 定时任务在前，无时段在后
        assert_eq!(plan.items[0].task_name, "晨跑");
        assert_eq!(plan.items[1].task_name, "读书");
        assert!(plan.items[1].start.is_none());
    }

    #[test]
    fn untimed_tasks_sorted_by_priority_desc() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let low = Task {
            id: 1,
            name: "低".into(),
            description: None,
            frequency: Frequency::Daily { times_per_day: 1 },
            slots: vec![],
            priority_level: PriorityLevel::Low,
            active: true,
        };
        let urgent = Task {
            id: 2,
            name: "急".into(),
            description: None,
            frequency: Frequency::Daily { times_per_day: 1 },
            slots: vec![],
            priority_level: PriorityLevel::Urgent,
            active: true,
        };
        let plan = build_day_plan(date, &[low, urgent]);
        assert_eq!(plan.items[0].task_name, "急");
        assert_eq!(plan.items[1].task_name, "低");
    }
}
