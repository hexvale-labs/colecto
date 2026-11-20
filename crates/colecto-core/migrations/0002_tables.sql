-- Drafts, labels, API keys and send approvals get their own tables instead of one JSON
-- record each, so a change writes one row and a later migration can alter one column.
-- The v1 records are converted here and then removed.

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
-- `snapshot` is the reviewed message as JSON; `status` is pending, rejected, expired,
-- submitting, sent or uncertain.
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

INSERT INTO drafts(account, id, recipient, cc, subject, body, in_reply_to, message_references, forward_of)
SELECT substr(r.namespace, 9),
       CAST(substr(json_extract(d.value, '$.id'), 7) AS INTEGER),
       json_extract(d.value, '$.recipient'),
       coalesce(json_extract(d.value, '$.cc'), ''),
       json_extract(d.value, '$.subject'),
       json_extract(d.value, '$.body'),
       coalesce(json_extract(d.value, '$.in_reply_to'), ''),
       coalesce(json_extract(d.value, '$.references'), ''),
       json_extract(d.value, '$.forward_of')
FROM records r, json_each(CAST(r.value AS TEXT), '$.drafts') d
WHERE r.namespace LIKE 'account:%' AND r.name = 'drafts';

INSERT INTO labels(account, name)
SELECT substr(r.namespace, 9), l.key
FROM records r, json_each(CAST(r.value AS TEXT), '$.labels') l
WHERE r.namespace LIKE 'account:%' AND r.name = 'labels';

INSERT INTO label_messages(account, label, message)
SELECT substr(r.namespace, 9), l.key, m.value
FROM records r, json_each(CAST(r.value AS TEXT), '$.labels') l, json_each(l.value) m
WHERE r.namespace LIKE 'account:%' AND r.name = 'labels';

INSERT INTO api_keys(id, name, accounts, capabilities, created_at, expires_at, last_used_at, revoked_at, verifier)
SELECT json_extract(k.value, '$.metadata.id'),
       json_extract(k.value, '$.metadata.name'),
       json(json_extract(k.value, '$.metadata.accounts')),
       json(json_extract(k.value, '$.metadata.capabilities')),
       json_extract(k.value, '$.metadata.created_at'),
       json_extract(k.value, '$.metadata.expires_at'),
       json_extract(k.value, '$.metadata.last_used_at'),
       json_extract(k.value, '$.metadata.revoked_at'),
       unhex((SELECT group_concat(printf('%02x', b.value), '')
              FROM json_each(k.value, '$.verifier') b))
FROM records r, json_each(CAST(r.value AS TEXT), '$.keys') k
WHERE r.namespace = 'host' AND r.name = 'api-keys';

INSERT INTO send_approvals(id, requester, account, sender, snapshot, created_at, expires_at, status)
SELECT json_extract(a.value, '$.id'),
       json_extract(a.value, '$.requester'),
       json_extract(a.value, '$.account'),
       json_extract(a.value, '$.from'),
       json(json_extract(a.value, '$.snapshot')),
       json_extract(a.value, '$.created_at'),
       json_extract(a.value, '$.expires_at'),
       json_extract(a.value, '$.status')
FROM records r, json_each(CAST(r.value AS TEXT), '$.requests') a
WHERE r.namespace = 'host' AND r.name = 'send-approvals';

DELETE FROM records
WHERE (namespace LIKE 'account:%' AND name IN ('drafts', 'labels'))
   OR (namespace = 'host' AND name IN ('api-keys', 'send-approvals'));
