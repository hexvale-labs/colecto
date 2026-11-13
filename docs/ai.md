# AI features

Optional summaries, drafted replies and automatic labels, using AI services the owner
configures. Everything is off by default, and while it is off Colecto makes no AI
request and shows no AI interface outside Settings → AI.

## What is sent where

| Feature | Sent | Endpoint |
| --- | --- | --- |
| Summarize | Sender, subject and up to 24,000 characters of the open message's text | `POST {text base URL}/chat/completions` |
| Reply | The same, plus the owner's notes (at most 4,000 characters) and the chosen tone | `POST {text base URL}/chat/completions` |
| Automatic labels: model | Sender, subject and up to 8,000 characters of each newly arrived message | `POST {decision base URL}/api/alpha/decisions` |
| Automatic labels: rules | Nothing; rules run in the Host | — |
| Test connection | A fixed test text | The model being tested |

The Host makes every request; the browser never sees an API key or talks to the
service. Prompts, mail content, model answers and keys are never logged; a refused
request logs only its HTTP status. Errors shown to the owner are fixed plain-language
messages (for example "The AI service rejected the API key."), never the service's own
response, which could echo the prompt. Requests have a 10-second connect timeout and a
120-second overall timeout, follow no redirects, and read at most 2 MiB of response.

## Settings → AI

Navigation id `ai` (`settings/ai` for `app.navigate`). The page belongs to the bundled
`ai` extension (`ui/src/extensions/ai/AiSettings.svelte`); the mail shell renders it.

| Setting | Default | Limits and meaning |
| --- | --- | --- |
| AI features (`enabled`) | Off | Master switch; everything below is visible but inactive while off |
| Text model base URL | empty | http or https, no credentials, query or fragment; at most 2048 bytes |
| Text model id | empty | At most 200 characters |
| Max tokens | 1024 | 16–32000, sent as `max_tokens` |
| Thinking effort | Default | `none`, `low`, `medium`, `high`; sent as `reasoning_effort` only when set |
| Decision base URL | `https://openrouter.ai` | As above |
| Decision model id | `typesafe/jev-1.14` | `cloudflare/clef` uses the same request shape |
| Confidence needed (`threshold`) | 0.70 | 0.50–0.99 |
| Automatic labels (`autolabel`) | Off | Runs only while AI features are also on |

API keys are write-only. `POST /ai/keys` stores or removes one; the settings response
only says whether a key is saved. Keys are optional: without one, no `Authorization`
header is sent, which suits services running on the owner's own computer.

## Requests and answers

Text model, OpenAI-compatible chat completions:

```json
{"model":"…","messages":[{"role":"system","content":"…"},{"role":"user","content":"…"}],"max_tokens":1024,"reasoning_effort":"low"}
```

The answer is `choices[0].message.content`, a string or a list of `{text}` parts. A
leading `<think>…</think>` block is dropped. An empty answer is an error ("raise max
tokens"), except for Test connection, where any readable answer counts.

Decision model, the [OpenRouter Decisions API](https://openrouter.ai/docs/guides/community/jev)
with bearer authentication. One Noul (yes/no) question per label the model is asked about:

```json
{"model":"typesafe/jev-1.14","state":["From: Maya <maya@example.test>\nSubject: …\n\n…"],
 "questions":{"q0":{"type":"noul","instructions":"This email belongs under the label \"Receipts\". The label is for: …"}}}
```

`answers.q0` is `{"type":"noul","noul":0.91}` (the probability of yes) or
`{"type":"refusal"}`. Missing, refused, non-numeric or out-of-range answers are ignored.
A label applies when its probability is at least the threshold, so one message can get
several labels.

## Prompts and tones

The system prompts live in `crates/colecto-host/src/ai.rs`. Both tell the model the
email is data, not instructions. The reply prompt keeps the owner's meaning exactly,
adds no promises or facts, writes only the body and answers in the original email's
language unless the notes say otherwise. Each tone adds one short instruction:

| Tone (`tone`) | Instruction in brief |
| --- | --- |
| Casual (`casual`) | Relaxed and friendly |
| Lazy (`lazy`) | All lowercase, minimal punctuation, an occasional "lol"/"haha"/"lmfao", not overdone |
| Professional (`professional`) | Formal, no slang, correct punctuation |
| Roast (`roast`) | Comedian riffing on the recipient: self-deprecating opener, then light jabs; never cruel |
| Troll (`troll`) | For scams and ragebait: "lol", "ig", some aLtErNaTiNg CaSe, emojis; readable, never hateful |

## Reader actions

While AI features are on and the `ai` extension is enabled, the reader toolbar shows a
plain **AI** button next to Reply, Reply all and Forward. Its menu (arrow keys, Home,
End, Escape) offers **Summarize** and, when the composer is on, **Reply**.

- **Summarize** shows a Summary block above the message body with **Regenerate** and
  **Delete**. Summaries are kept per account and message in the Host, so reopening the
  message shows the saved one without a new request.
- **Reply** opens a dialog: the owner writes the gist in any language, picks a tone on a
  five-part segmented control (each segment explains itself on hover or keyboard focus;
  arrow keys move between tones), presses **Generate**, can edit the draft, and **Use as
  reply** opens the normal reply composer with the draft above the quoted message.
  Notes and the draft stay while the message is open.

The reader does not import the AI extension; see "Reader slots" in the
[extension contract](extensions/README.md#reader-slots).

## Automatic labels

Labels are local and per account, so rules are kept per account and label name. Each
label has an on/off switch, up to 10 rules, an optional description (context for the
model, at most 500 characters) and a switch to also ask the decision model about mail
no rule matched.

| Rule part | Meaning |
| --- | --- |
| `field` | `sender` (name or address), `subject`, `body` or `any` |
| `pattern` | A case-insensitive [Rust `regex`](https://docs.rs/regex) pattern, at most 300 bytes; plain words work as they are |

Patterns compile with 1 MiB size limits and run in linear time. Settings with an invalid
pattern are refused with the label's name. The label editor tests its rules against the
account's saved Inbox (`POST /ai/labels/preview`) as the owner types, without any
network request, and shows how many messages match and the first five.

Labelling runs in the background refresh (`crates/colecto-host/src/service.rs`) when it
sees new message ids, which includes IMAP IDLE and JMAP push refreshes. It needs AI
features and automatic labels on. Rules apply first; the model is asked only about the
remaining model-enabled labels.

| Limit | Value |
| --- | --- |
| Model requests per account and refresh | 10 messages (one request each) |
| Model requests per hour, all accounts | 120 |
| Remembered handled messages per account | 2,000 |

Each message is handled at most once (the `ai-autolabel-seen` record). Removing a label
from a message by hand also marks it handled, so automatic labelling never re-applies a
label the owner removed. Mail that arrived while the Host was not running is not
labelled, because the first refresh after start only sets the baseline. Labels that no
longer exist are skipped. A failed model request stops model requests for the rest of
that refresh; rules still apply.

## HTTP routes

All under `/api/v1`, owner bearer only. Scoped API keys are refused by the
authentication layer, and Wasm extensions never receive the HTTP path.

| Route | Caller (`x-colecto-extension`) | Purpose |
| --- | --- | --- |
| `GET`/`POST /ai/settings` | `host` | Read or replace the settings; returns `{settings, keys:{text,decision}}` |
| `POST /ai/keys` | `host` | `{kind:"text"\|"decision", key:string\|null}` |
| `POST /ai/test` | `host` | `{kind}`; checks the saved connection |
| `POST /ai/labels/preview` | `host` (+ `x-colecto-account`) | `{rule}` → `{errors, checked, count, matches}` |
| `GET`/`POST`/`DELETE /ai/summaries/{id}` | `ai`, needs `messages.read` | Read, create or delete the saved summary |
| `POST /ai/reply` | `ai`, needs `messages.read` | `{message_id, input, tone}` → `{text}` |

The `ai` routes also require the extension to be enabled, and answer HTTP 409 while AI
features are off. A locked credential vault answers 409 ("Unlock your credentials…").

## Storage

| Record (`host` namespace) | Contents | Bound |
| --- | --- | --- |
| `ai` | Settings and label rules | Validated as above |
| `ai-summaries` | `{account, message, text, created_at}` | 1,000 summaries of at most 4,000 characters; oldest dropped first |
| `ai-autolabel-seen` | Handled message ids per account | 2,000 per account |

API keys are `host:ai-text` and `host:ai-decision` secrets in the encrypted credential
vault beside account credentials (see [accounts](accounts.md)). Locking the vault makes
them unavailable; **Reset saved credentials** deletes them. Removing an account deletes
its label rules, summaries and handled ids. Summaries are plaintext, like mail snapshots.
