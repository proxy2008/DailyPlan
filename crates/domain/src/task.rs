//! 任务定义：频率、时段、任务实体。

use chrono::{Datelike, NaiveDate, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};

/// 一天里的一个时间段，如 `06:30–07:00`。
/// 半开区间：`[start, end)`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSlot {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl TimeSlot {
    /// 时长（分钟）。若 end <= start 视为跨午夜，MVP 暂不支持，返回 0。
    pub fn duration_minutes(&self) -> u32 {
        if self.end <= self.start {
            return 0;
        }
        let secs = (self.end - self.start).num_seconds();
        (secs / 60) as u32
    }
}

/// 任务优先级（4 档）。用于排序与显示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLevel {
    Urgent,
    High,
    Normal,
    Low,
}

impl Default for PriorityLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl PriorityLevel {
    /// 数值越大越优先（用于排序）。
    pub fn rank(&self) -> i32 {
        match self {
            Self::Urgent => 3,
            Self::High => 2,
            Self::Normal => 1,
            Self::Low => 0,
        }
    }

    /// 整数 rank 反解（越界值 clamp）。
    pub fn from_rank(r: i32) -> Self {
        match r {
            r if r >= 3 => Self::Urgent,
            2 => Self::High,
            1 => Self::Normal,
            _ => Self::Low,
        }
    }

    pub fn label_cn(&self) -> &'static str {
        match self {
            Self::Urgent => "紧急",
            Self::High => "重要",
            Self::Normal => "一般",
            Self::Low => "可选",
        }
    }
}

/// 任务出现的频率。MVP 支持四种；月度/日期范围/软调度留给二期。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum Frequency {
    /// 每天出现，一天内重复 N 次（对应 N 个 slot）。
    Daily { times_per_day: u32 },
    /// 每周指定日出现。`weekdays[0]` = 周一, `[6]` = 周日。
    Weekly { weekdays: [bool; 7] },
    /// 每 N 天一次，从 `start` 起算。
    Interval { every_days: u32, start: NaiveDate },
    /// 仅在某一天出现（一次性）。
    Once { date: NaiveDate },
    /// 用户手动指定的若干日期（保持升序+去重）。
    Custom { dates: Vec<NaiveDate> },
}

impl Default for Frequency {
    fn default() -> Self {
        Frequency::Daily { times_per_day: 1 }
    }
}

impl Frequency {
    /// 判断该任务在 `date` 这一天是否应该出现。
    pub fn matches(&self, date: NaiveDate) -> bool {
        match self {
            Frequency::Daily { .. } => true,
            Frequency::Weekly { weekdays } => {
                let idx = weekday_to_index(date.weekday());
                weekdays[idx]
            }
            Frequency::Interval { every_days, start } => {
                if date < *start || *every_days == 0 {
                    return false;
                }
                // 约定：start 当天算第 0 天，之后每 every_days 天一次。
                let elapsed = (date - *start).num_days();
                elapsed % (*every_days as i64) == 0
            }
            Frequency::Once { date: d } => date == *d,
            Frequency::Custom { dates } => dates.binary_search(&date).is_ok(),
        }
    }
}

/// 周一=0 .. 周日=6，与 `Frequency::Weekly` 的数组下标对齐。
pub(crate) fn weekday_to_index(d: Weekday) -> usize {
    match d {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    }
}

/// 一个任务定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub frequency: Frequency,
    /// 绑定的时间段（硬绑定）。Daily 频率下通常每个 slot 对应一次出现。
    #[serde(default)]
    pub slots: Vec<TimeSlot>,
    /// 冲突时谁让位；级别越高越优先。
    #[serde(default)]
    pub priority_level: PriorityLevel,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            description: None,
            frequency: Frequency::default(),
            slots: Vec::new(),
            priority_level: PriorityLevel::default(),
            active: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn slot(start: &str, end: &str) -> TimeSlot {
        TimeSlot {
            start: NaiveTime::parse_from_str(start, "%H:%M").unwrap(),
            end: NaiveTime::parse_from_str(end, "%H:%M").unwrap(),
        }
    }

    #[test]
    fn slot_duration() {
        assert_eq!(slot("06:30", "07:00").duration_minutes(), 30);
        assert_eq!(slot("07:00", "06:30").duration_minutes(), 0); // 倒序
    }

    #[test]
    fn daily_matches_every_day() {
        let f = Frequency::Daily { times_per_day: 1 };
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 4).unwrap()));
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()));
    }

    #[test]
    fn weekly_matches_chosen_days() {
        // 周一、三、五
        let f = Frequency::Weekly {
            weekdays: [true, false, true, false, true, false, false],
        };
        // 2026-07-04 是周六 -> 不命中
        assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 4).unwrap()));
        // 2026-07-06 是周一 -> 命中
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 6).unwrap()));
        // 2026-07-08 是周三 -> 命中
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 8).unwrap()));
    }

    #[test]
    fn interval_matches_every_n_days() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let f = Frequency::Interval {
            every_days: 3,
            start,
        };
        // start 当天算第 0 天：7/1, 7/4, 7/7 命中
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()));
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 4).unwrap()));
        assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()));
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 7).unwrap()));
    }

    #[test]
    fn once_matches_only_that_day() {
        let f = Frequency::Once {
            date: NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
        };
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()));
        assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 11).unwrap()));
    }

    #[test]
    fn custom_matches_chosen_dates() {
        let f = Frequency::Custom {
            dates: vec![
                NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 8).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 12).unwrap(),
            ],
        };
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()));
        assert!(f.matches(NaiveDate::from_ymd_opt(2026, 7, 12).unwrap()));
        assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 6).unwrap()));
    }

    #[test]
    fn custom_empty_matches_nothing() {
        let f = Frequency::Custom { dates: vec![] };
        assert!(!f.matches(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()));
    }

    #[test]
    fn priority_level_rank_roundtrip() {
        for orig in [PriorityLevel::Urgent, PriorityLevel::High, PriorityLevel::Normal, PriorityLevel::Low] {
            let back = PriorityLevel::from_rank(orig.rank());
            assert_eq!(orig, back, "{:?} 往返不一致", orig);
        }
    }

    #[test]
    fn priority_level_from_rank_clamps() {
        assert_eq!(PriorityLevel::from_rank(99), PriorityLevel::Urgent);
        assert_eq!(PriorityLevel::from_rank(-5), PriorityLevel::Low);
    }
}
