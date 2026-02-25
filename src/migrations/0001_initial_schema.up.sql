CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT DEFAULT '',
    type        TEXT DEFAULT 'task',
    priority    INTEGER DEFAULT 2,
    status      TEXT DEFAULT 'open',
    parent      TEXT DEFAULT '',
    created     TEXT NOT NULL,
    updated     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
    task_id TEXT,
    label   TEXT,
    PRIMARY KEY (task_id, label),
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

CREATE TABLE IF NOT EXISTS blockers (
    task_id    TEXT,
    blocker_id TEXT,
    PRIMARY KEY (task_id, blocker_id),
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

CREATE INDEX IF NOT EXISTS idx_status   ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_priority ON tasks(priority);
CREATE INDEX IF NOT EXISTS idx_parent   ON tasks(parent);
