-- Revert labels/blockers foreign keys to definitions without ON DELETE CASCADE.

CREATE TABLE labels_old (
    task_id TEXT,
    label   TEXT,
    PRIMARY KEY (task_id, label),
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

INSERT INTO labels_old (task_id, label)
    SELECT task_id, label FROM labels;

DROP TABLE labels;

ALTER TABLE labels_old RENAME TO labels;

CREATE TABLE blockers_old (
    task_id    TEXT,
    blocker_id TEXT,
    PRIMARY KEY (task_id, blocker_id),
    FOREIGN KEY (task_id) REFERENCES tasks(id),
    FOREIGN KEY (blocker_id) REFERENCES tasks(id)
);

INSERT INTO blockers_old (task_id, blocker_id)
    SELECT task_id, blocker_id FROM blockers;

DROP TABLE blockers;

ALTER TABLE blockers_old RENAME TO blockers;
