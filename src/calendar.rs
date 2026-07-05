//! 内联月历网格组件：多选日期。

use chrono::{Datelike, NaiveDate, Weekday};
use leptos::prelude::*;

/// 把 chrono Weekday 转为 周一=0..周日=6 的索引。
/// 复制自 domain::task::weekday_to_index（那里是 pub(crate)，跨 crate 不能直接用）。
fn weekday_to_index(d: Weekday) -> usize {
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

/// 生成某月的 6×7 网格日期（含前后月补齐）。
fn month_grid(year: i32, month: u32) -> Vec<Option<NaiveDate>> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let leading = weekday_to_index(first.weekday());
    let mut days: Vec<Option<NaiveDate>> = (0..leading).map(|_| None).collect();
    let mut d = first;
    while d.month() == month {
        days.push(Some(d));
        d = d.succ_opt().unwrap();
    }
    while days.len() % 7 != 0 {
        days.push(None);
    }
    days
}

/// 月历组件。selected 是外部持有的选中日期信号。
#[component]
pub fn Calendar(selected: RwSignal<Vec<NaiveDate>>) -> impl IntoView {
    let view_year = RwSignal::new(chrono::Local::now().date_naive().year());
    let view_month = RwSignal::new(chrono::Local::now().date_naive().month());

    let prev_month = move || {
        view_month.update(|m| {
            if *m == 1 {
                *m = 12;
                view_year.update(|y| *y -= 1);
            } else {
                *m -= 1;
            }
        });
    };
    let next_month = move || {
        view_month.update(|m| {
            if *m == 12 {
                *m = 1;
                view_year.update(|y| *y += 1);
            } else {
                *m += 1;
            }
        });
    };

    const HEADERS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

    view! {
        <div class="calendar">
            <div class="calendar-nav">
                <button type="button" on:click=move |_| prev_month()>"‹"</button>
                <span>{move || format!("{} 年 {} 月", view_year.get(), view_month.get())}</span>
                <button type="button" on:click=move |_| next_month()>"›"</button>
            </div>
            <div class="calendar-grid">
                {HEADERS.iter().copied().map(|h| view! { <div class="calendar-cell header">{h}</div> }).collect::<Vec<_>>()}
                {move || {
                    let grid = month_grid(view_year.get(), view_month.get());
                    let sel = selected.get();
                    grid.into_iter().map(|d_opt| {
                        match d_opt {
                            None => view! { <div class="calendar-cell empty"></div> }.into_any(),
                            Some(d) => {
                                let is_selected = sel.contains(&d);
                                let d_for_click = d;
                                view! {
                                    <button type="button" class="calendar-cell" class:selected=move || is_selected
                                        on:click=move |_| {
                                            selected.update(|s| {
                                                if let Some(pos) = s.iter().position(|x| *x == d_for_click) {
                                                    s.remove(pos);
                                                } else {
                                                    s.push(d_for_click);
                                                    s.sort();
                                                    s.dedup();
                                                }
                                            });
                                        }>
                                        {d.day()}
                                    </button>
                                }.into_any()
                            }
                        }
                    }).collect::<Vec<_>>().into_any()
                }}
            </div>
            <div class="calendar-summary">
                {move || format!("已选 {} 个日期", selected.get().len())}
                <button type="button" on:click=move |_| selected.update(|s| s.clear())>"清空"</button>
            </div>
        </div>
    }.into_any()
}
