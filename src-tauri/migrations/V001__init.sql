-- DailyPlan 初始 schema。
-- frequency 和 slots 存 JSON 文本列（domain crate 用 serde_json (de)serialize）。

CREATE TABLE IF NOT EXISTS tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    description TEXT,
    frequency   TEXT    NOT NULL,   -- JSON: dailyplan_domain::Frequency
    slots       TEXT    NOT NULL,   -- JSON: Vec<dailyplan_domain::TimeSlot>
    priority    INTEGER NOT NULL DEFAULT 0,
    active      INTEGER NOT NULL DEFAULT 1,  -- 0/1 布尔
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tasks_active ON tasks(active);
