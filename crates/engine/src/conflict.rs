//! 冲突检测：两个时段是否重叠。
//!
//! 区间模型：半开 `[start, end)`。首尾相接不算冲突。
//! 重叠条件：`s1 < e2 && s2 < e1`。

use dailyplan_domain::checklist::{ChecklistItem, Conflict};

/// 对已排序（按 start 升序）的 items 做两两重叠检测，返回 Conflict 列表。
/// MVP 用 O(n²) 简单实现；一天的事项通常 < 50，足够。
pub fn detect_conflicts(items: &[ChecklistItem]) -> Vec<Conflict> {
    let mut out = Vec::new();
    let n = items.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let a = &items[i];
            let b = &items[j];
            // 已按 start 排序，b.start >= a.start；若 b.start >= a.end 则后续都不可能重叠，可提前 break。
            if b.start >= a.end {
                break;
            }
            // 走到这里说明 b.start < a.end，又因 b.start >= a.start，所以一定重叠。
            out.push(Conflict {
                item_a: i,
                item_b: j,
                message: format!(
                    "“{}”({}-{})与“{}”({}-{})时段重叠",
                    a.task_name,
                    a.start.format("%H:%M"),
                    a.end.format("%H:%M"),
                    b.task_name,
                    b.start.format("%H:%M"),
                    b.end.format("%H:%M")
                ),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn item(name: &str, s: &str, e: &str) -> ChecklistItem {
        ChecklistItem {
            task_id: 0,
            task_name: name.into(),
            start: NaiveTime::parse_from_str(s, "%H:%M").unwrap(),
            end: NaiveTime::parse_from_str(e, "%H:%M").unwrap(),
            duration_min: 0,
        }
    }

    #[test]
    fn no_items_no_conflicts() {
        assert!(detect_conflicts(&[]).is_empty());
    }

    #[test]
    fn overlap_detected() {
        let items = vec![
            item("A", "07:00", "08:30"),
            item("B", "08:00", "09:00"),
        ];
        let c = detect_conflicts(&items);
        assert_eq!(c.len(), 1);
        assert!(c[0].message.contains("A") && c[0].message.contains("B"));
    }

    #[test]
    fn back_to_back_no_conflict() {
        let items = vec![
            item("A", "07:00", "08:00"),
            item("B", "08:00", "09:00"),
        ];
        assert!(detect_conflicts(&items).is_empty());
    }

    #[test]
    fn three_overlapping_chain() {
        let items = vec![
            item("A", "07:00", "08:30"),
            item("B", "07:30", "08:00"),
            item("C", "08:00", "09:00"), // 与 A 重叠（A 到 08:30），与 B 首尾相接
        ];
        let c = detect_conflicts(&items);
        // A-B 重叠, A-C 重叠, B-C 不重叠（B 08:00 结束，C 08:00 开始）
        assert_eq!(c.len(), 2);
    }
}
