-- Add ON DELETE CASCADE to labels/task_id and blockers/task_id+blocker_id.
-- SQLite has no ALTER TABLE ADD CONSTRAINT, so we rebuild both tables.

-- Drop dangling label rows before introducing stricter FK behavior.
DELETE FROM labels WHERE task_id NOT IN (SELECT id FROM tasks);

CREATE TABLE labels_new (
    task_id TEXT,
    label   TEXT,
    PRIMARY KEY (task_id, label),
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

INSERT INTO labels_new (task_id, label)
    SELECT task_id, label FROM labels;

DROP TABLE labels;

ALTER TABLE labels_new RENAME TO labels;

CREATE TABLE blockers_new (
    task_id    TEXT,
    blocker_id TEXT,
    PRIMARY KEY (task_id, blocker_id),
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (blocker_id) REFERENCES tasks(id) ON DELETE CASCADE
);

INSERT INTO blockers_new (task_id, blocker_id)
    SELECT task_id, blocker_id FROM blockers;

DROP TABLE blockers;

ALTER TABLE blockers_new RENAME TO blockers;
