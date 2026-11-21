-- Native AI summaries and auto-label tracking (first-party, stored in colecto.db).
CREATE TABLE IF NOT EXISTS ai_summaries(
    account TEXT NOT NULL,
    message_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(account, message_id)
);
CREATE TABLE IF NOT EXISTS ai_autolabel_seen(
    account TEXT NOT NULL,
    message_id TEXT NOT NULL,
    seen_at INTEGER NOT NULL,
    PRIMARY KEY(account, message_id)
);
