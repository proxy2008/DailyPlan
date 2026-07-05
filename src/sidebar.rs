//! 左侧导航栏：在「日程」和「任务管理」两个页面之间切换。
//!
//! 设计：200px 宽，内嵌 SVG 图标（不依赖 emoji，跨平台一致），
//! 顶部 App 名称，导航项图标+文字，当前项用主色背景高亮。

use leptos::prelude::*;

/// 当前展示哪个页面。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Schedule,
    TaskManage,
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Self::Schedule => "日程",
            Self::TaskManage => "任务管理",
        }
    }

    /// 内嵌 SVG path（24x24 viewBox，stroke 风格，类似 Lucide/Feather）。
    fn icon_path(self) -> &'static str {
        match self {
            // 日历图标
            Self::Schedule => "M3 9h18M6 3v3M18 3v3M5 5h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
            // 勾选列表图标
            Self::TaskManage => "M9 6h11M9 12h11M9 18h11M4 6l1.5 1.5L8 4M4 12l1.5 1.5L8 10M4 18l1.5 1.5L8 16",
        }
    }
}

/// 渲染 SVG 图标（stroke 风格，跟随 currentColor）。
fn Icon(path: &'static str) -> impl IntoView {
    view! {
        <svg class="nav-icon" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d=path />
        </svg>
    }
}

/// 左侧导航栏组件。
#[component]
pub fn Sidebar(page: RwSignal<Page>) -> impl IntoView {
    let pages = [Page::Schedule, Page::TaskManage];
    view! {
        <nav class="sidebar">
            // 顶部：App 标识
            <div class="sidebar-brand">
                <div class="brand-mark">"DP"</div>
                <div class="brand-name">"DailyPlan"</div>
            </div>

            // 导航项
            <div class="nav-list">
                {pages.iter().map(|&p| {
                    let label = p.label();
                    let path = p.icon_path();
                    let is_active = move || page.get() == p;
                    view! {
                        <button
                            type="button"
                            class="nav-item"
                            class:active=is_active
                            on:click=move |_| page.set(p)
                        >
                            {Icon(path)}
                            <span class="nav-label">{label}</span>
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </nav>
    }
}
