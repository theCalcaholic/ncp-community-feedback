CREATE TABLE IF NOT EXISTS feedback_staged_rollouts (
    id INTEGER PRIMARY KEY NOT NULL,
    date DATE NOT NULL,
    ip_hash TEXT NOT NULL,
    UNIQUE(date, ip_hash)
);
