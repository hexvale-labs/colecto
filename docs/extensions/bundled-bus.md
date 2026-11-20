# Bundled bus surfaces

The bus requests and events each bundled extension offers, with their versions. Bus
versions are separate from [API versions](api-versions/README.md): they describe how
extensions talk to each other, not how they talk to Colecto.

## Versioning rules for bundled extensions

- Every request and event name starts with a whole-number version: `v1/add_button`,
  `v2/add_button`. Never `v1.5/add_button` and never a bare `add_button`.
- A compatible change, such as a new optional payload field, keeps the name. Write it
  in the table below.
- An incompatible change adds `v<n+1>/<name>` next to the old one. Keep the old name
  working until nothing bundled uses it, then remove it in a release that says so.
- When you add, change or remove a name, update the table in the same change.

## Surfaces

| Bus | Name | Kind | Payload → answer | Since |
| --- | --- | --- | --- | --- |
| `composer` | `v1/open` | Request | `{draft_id}` opens a saved draft, `{draft:{recipient?,subject?,body?,…}}` a new one with those fields, `{}` a blank one | API 1 |
| `composer` | `v1/get_draft` | Request | `null` → `{recipient,subject,body}`, or null when closed | API 1 |
| `composer` | `v1/set_draft` | Request | `{recipient?,subject?,body?}` | API 1 |
| `composer` | `v1/opened` | Event | `{}` | API 1 |
| `composer` | `v1/editing` | Event | `{draft_id}`, or `null` when closed or not yet saved | API 1 |
| `composer` | `v1/saved` | Event | `{draft_id}` | API 1 |
| `composer` | `v1/sending` | Event | `{}` (informational) | API 1 |
| `reader` | `v1/get_html` | Request | `{message_id}` | API 1 |
| `reader` | `v1/set_html` | Request | `{message_id,html}` | API 1 |
| `reader` | `v1/add_banner` | Request | `{message_id,text,tone:"info"\|"warning"}` | API 1 |
| `reader` | `v1/reply` | Request | `{message_id,body}`: start a reply to the shown message with `body` above the quote | API 1 |
| `reader` | `v1/message_opened` | Event | `{message_id,subject,from,html,text}` | API 1 |
| `reader` | `v1/viewing` | Event | `{account,message_id}` (`null` when nothing is shown) | API 1 |
| `reader` | `v1/labels` | Event | `{account}`: the reader changed labels | API 1 |
| `inbox` | `v1/open` | Request | `{account?,message_id}`: switch to Inbox and open the message | API 1 |
| `inbox` | `v1/selected` | Event | `{account,message_id,folder,subject}`: `folder` is the name shown, such as `Inbox` or a provider folder's name | API 1 |
| `inbox` | `v1/listed` | Event | `{account,folder,ids}`: the first rows to prefetch; `folder` is `inbox`, `sent` or a provider folder id | API 1 |
| `inbox` | `v1/labels` | Event | `{account}`: the inbox changed labels | API 1 |
| `inbox` | `v1/add_row_action` | Request | `{id,label,icon?}` → `true`: add an entry to every message's menu; at most 4 per extension, labels cut to 64 characters | API 1 |
| `inbox` | `v1/remove_row_action` | Request | `{id?}` → `true`: remove one of the caller's entries, or all of them | API 1 |
| `inbox` | `v1/row_action` | Event | `{extension,action,account,message_id,folder}`: someone picked an added entry; only `extension` should act. `folder` is `inbox`, `sent`, `drafts` or a provider folder id | API 1 |
| `inbox` | `v1/ready` | Event | `{}`: the inbox started; add row actions again | API 1 |
| `notifications` | `v1/notify` | Request | `{title,body}` | API 1 |
| `ai` | `v1/labeled` | Event | `{account}`: the AI service part auto-labelled new mail | API 1 |
| `ai` | `v1/reclassify` | Request | `{account,ids}` (at most 25) → `{checked,changed,unanswered}`: label these messages again, replacing the AI's earlier labels; `unanswered` counts messages the decision model's limit skipped | API 1 |

| Slot | Context | Shown |
| --- | --- | --- |
| `reader/v1/actions` | `{account,message_id}` | In the reader's toolbar |
| `reader/v1/above_body` | `{account,message_id}` | Between the header and the message body |
| `composer/v1/actions` | `{draft_id}` | Next to the composer's Send and Save buttons |

Slots take `contribute` trees from extensions with `ui.contribute`. `composer`'s
`v1/add_button` and `v1/remove_button` are gone: an extension contributes to
`composer/v1/actions` instead, and reads or changes the draft with `v1/get_draft` and
`v1/set_draft`.

## Behaviour

Draft requests return null when the composer is closed. Reader awaits event delivery
before displaying the body. HTML replacements stay in its sandboxed iframe and show
“<extension name> changed how this message is shown”; banners are text only (at most
280 characters, the latest three kept). Notifications label the calling extension.
