-- Add FOREIGN KEY (blocker_id) REFERENCES tasks(id) to the blockers table.
-- SQLite has no ALTER TABLE ADD CONSTRAINT, so we rebuild.

-- Drop dangling blocker_id rows that reference nonexistent tasks.
DELETE FROM blockers WHERE blocker_id NOT IN (SELECT id FROM tasks);

CREATE TABLE blockers_new (
    task_id    TEXT,
    blocker_id TEXT,
    PRIMARY KEY (task_id, blocker_id),
    FOREIGN KEY (task_id)    REFERENCES tasks(id),
    FOREIGN KEY (blocker_id) REFERENCES tasks(id)
);

INSERT INTO blockers_new (task_id, blocker_id)
    SELECT task_id, blocker_id FROM blockers;

DROP TABLE blockers;

ALTER TABLE blockers_new RENAME TO blockers;
