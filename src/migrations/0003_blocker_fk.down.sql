-- Revert to the original blockers table without the blocker_id FK.

CREATE TABLE blockers_old (
    task_id    TEXT,
    blocker_id TEXT,
    PRIMARY KEY (task_id, blocker_id),
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

INSERT INTO blockers_old (task_id, blocker_id)
    SELECT task_id, blocker_id FROM blockers;

DROP TABLE blockers;

ALTER TABLE blockers_old RENAME TO blockers;
