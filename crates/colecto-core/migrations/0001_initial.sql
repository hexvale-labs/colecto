-- Colecto beta schema. Never edit a migration that has shipped; add the next number instead.

-- Small typed documents owned by Core and the Host, such as the credential vault, drafts
-- and the API key list. Callers validate their own serialized formats.
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
