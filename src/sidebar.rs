//! 左侧导航栏：在「日程」和「任务管理」两个页面之间切换。

use leptos::prelude::*;

/// 当前展示哪个页面。放在 app.rs 顶层作为全局信号，sidebar 写、main 读。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// 日程页：当日打卡表 + 打印。
    Schedule,
    /// 任务管理页：任务列表 + 搜索/筛选/排序。
    TaskManage,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Self::Schedule => "日程",
            Self::TaskManage => "任务",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Schedule => "🗓",
            Self::TaskManage => "📋",
        }
    }
}

/// 左侧导航栏组件。点击切换 `page` 信号。
#[component]
pub fn Sidebar(page: RwSignal<Page>) -> impl IntoView {
    view! {
        <nav class="sidebar">
            <button
                type="button"
                class="sidebar-btn"
                class:active=move || page.get() == Page::Schedule
                title="日程"
                on:click=move |_| page.set(Page::Schedule)
            >
                <span class="sidebar-icon">{Page::Schedule.icon()}</span>
                <span class="sidebar-label">{Page::Schedule.label()}</span>
            </button>
            <button
                type="button"
                class="sidebar-btn"
                class:active=move || page.get() == Page::TaskManage
                title="任务管理"
                on:click=move |_| page.set(Page::TaskManage)
            >
                <span class="sidebar-icon">{Page::TaskManage.icon()}</span>
                <span class="sidebar-label">{Page::TaskManage.label()}</span>
            </button>
        </nav>
    }
}
