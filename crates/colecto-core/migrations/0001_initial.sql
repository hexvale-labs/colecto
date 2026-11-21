-- Colecto initial schema. Never edit a migration that has shipped; add the next number instead.

-- Small typed documents owned by Core and Services, such as the credential vault.
-- Callers validate their own serialized formats.
CREATE TABLE records(
    namespace TEXT NOT NULL,
    name TEXT NOT NULL,
    value BLOB NOT NULL,
    PRIMARY KEY(namespace, name)
);

-- Settings the owner changed. Defaults live in code, so a missing row means "use the
-- current default". `value` is JSON.
CREATE TABLE settings(
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY(scope, key)
);

-- Caches below can be refetched from the mail server. A migration may empty them
-- instead of converting their rows.
CREATE TABLE mailbox_identity(
    account TEXT NOT NULL,
    provider TEXT NOT NULL,
    folder TEXT NOT NULL,
    identity TEXT NOT NULL,
    PRIMARY KEY(account, provider, folder)
);
CREATE TABLE snapshots(
    account TEXT NOT NULL,
    provider TEXT NOT NULL,
    folder TEXT NOT NULL,
    fetched_at INTEGER,
    messages TEXT NOT NULL,
    PRIMARY KEY(account, provider, folder)
);
CREATE TABLE message_content(
    account TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    stored_at INTEGER NOT NULL,
    size INTEGER NOT NULL,
    raw BLOB NOT NULL,
    PRIMARY KEY(account, provider, id)
);

-- `id` is the number in the draft ID `draft-<id>`.
CREATE TABLE drafts(
    account TEXT NOT NULL,
    id INTEGER NOT NULL,
    recipient TEXT NOT NULL,
    cc TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    in_reply_to TEXT NOT NULL,
    message_references TEXT NOT NULL,
    forward_of TEXT,
    PRIMARY KEY(account, id)
);
CREATE TABLE labels(
    account TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY(account, name)
);
-- Messages keep the order they were labelled in (rowid).
CREATE TABLE label_messages(
    account TEXT NOT NULL,
    label TEXT NOT NULL,
    message TEXT NOT NULL,
    PRIMARY KEY(account, label, message)
);

-- `accounts` and `capabilities` are JSON arrays. `verifier` is the SHA-256 of the secret;
-- the secret itself is never stored.
CREATE TABLE api_keys(
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    accounts TEXT NOT NULL,
    capabilities TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER,
    last_used_at INTEGER,
    revoked_at INTEGER,
    verifier BLOB NOT NULL
);
CREATE TABLE send_approvals(
    id TEXT NOT NULL PRIMARY KEY,
    requester TEXT NOT NULL,
    account TEXT NOT NULL,
    sender TEXT NOT NULL,
    snapshot TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    status TEXT NOT NULL
);

-- Native AI summaries and auto-label tracking (first-party, stored in colecto.db).
CREATE TABLE ai_summaries(
    account TEXT NOT NULL,
    message_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(account, message_id)
);
CREATE TABLE ai_autolabel_seen(
    account TEXT NOT NULL,
    message_id TEXT NOT NULL,
    seen_at INTEGER NOT NULL,
    PRIMARY KEY(account, message_id)
);
