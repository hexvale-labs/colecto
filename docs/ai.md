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
response, which could echo the prompt. Requests have a 10-second connect timeout and an
overall timeout set in Settings → AI → Requests (`timeout_seconds`, 5 to 600, default
120), which covers summaries, replies and automatic labels alike, follow no redirects, and read at most 2 MiB of response.

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

Each message's menu in the mailbox (right-click, Shift+F10 or the Menu key) also offers
**Summarize**, **Draft a reply with AI** and **Check labels again** while AI is on. The
first two open the message in the reader and then run there. The AI extension adds them
through the inbox's `v1/add_row_action` and hears the choice on `inbox/v1/row_action`.

The reader does not import the AI extension: AI fills the reader's `reader/v1/actions`
and `reader/v1/above_body` slots and starts replies with `reader/v1/reply` (see
[Bundled bus surfaces](extensions/bundled-bus.md)).

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

Labelling runs in the AI extension's service part (`ui/src/extensions/ai/service/`,
built to `service.wasm`), which the Host invokes with a `mail` event when the background
refresh (`crates/colecto-host/src/service.rs`) sees new message ids, including IMAP IDLE
and JMAP push refreshes. It works with no window open. It needs AI features and
automatic labels on. Rules apply first; the model is asked only about the remaining
model-enabled labels, through the `ai.decide` service operation.

| Limit | Value |
| --- | --- |
| Model requests per mail event | 10 messages (one request each) |
| Model requests per hour, all accounts | 120, counted by the Host |
| Remembered handled messages per account | 500, in the extension's own storage (`seen.<account>`) |
| Service requests per event | 64 (the service-part limit) |

Each message is handled at most once, so a label the owner removes is never re-applied.
Mail that arrived while the Host was not running is not labelled, because the first
refresh after start only sets the baseline. Labels that no longer exist are skipped. A
failed model request stops model requests for the rest of that event; rules still
apply. After labelling, it emits `ai/v1/labeled` with `{account}`.

### Checking labels again

**Check labels again** (in a message's menu, and in the reader's AI menu) and **Check mail
again** under Settings → AI (the newest 1 to 100 saved Inbox
messages) ask the service part to label messages that were already handled, through
`ai/v1/reclassify`. This needs AI features on but not the automatic-labels switch. Unlike
new mail, a check both adds labels that now apply and removes labels that no longer do,
for labels that have rules, including labels the owner set by hand. The client sends 10
messages per request (the service takes at most 25), and the same hourly model limit
applies; messages it could not ask about are counted and reported.

## HTTP routes

All under `/api/v1`. Scoped API keys are refused by the authentication layer. The AI
extension's client part calls them with its own credential; `/ai/keys` stays owner-only
and is reached only from the trusted key field the shell draws.

| Route | Caller | Purpose |
| --- | --- | --- |
| `GET`/`POST /ai/settings` | Owner, or an extension with `ai.configure` | Read or replace the settings; returns `{settings, keys:{text,decision}}` |
| `POST /ai/keys` | Owner only | `{kind:"text"\|"decision", key:string\|null}` |
| `POST /ai/test` | Owner, or `ai.configure` | `{kind}`; checks the saved connection |
| `POST /ai/labels/preview` | Owner, or `ai.configure` (+ `x-colecto-account`) | `{rule}` → `{errors, checked, count, matches}` |
| `GET`/`POST`/`DELETE /ai/summaries/{id}` | An extension with `ai.text` and `messages.read` | Read, create or delete the saved summary |
| `POST /ai/reply` | An extension with `ai.text` and `messages.read` | `{message_id, input, tone}` → `{text}` |

Extension callers must be enabled, and the summary and reply routes answer HTTP 409 while AI
features are off. A locked credential vault answers 409 ("Unlock your credentials…").

## Storage

| Record (`host` namespace) | Contents | Bound |
| --- | --- | --- |
| `ai` | Settings and label rules | Validated as above |
| `ai-summaries` | `{account, message, text, created_at}` | 1,000 summaries of at most 4,000 characters; oldest dropped first |

API keys are `host:ai-text` and `host:ai-decision` secrets in the encrypted credential
vault beside account credentials (see [accounts](accounts.md)). Locking the vault makes
them unavailable; **Reset saved credentials** deletes them. Removing an account deletes
its label rules and summaries. Summaries are plaintext, like mail snapshots.
