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
| `composer` | `v1/add_button` | Request | `{id,label,request}` | v0.3.0 |
| `composer` | `v1/remove_button` | Request | `{id}` | v0.3.0 |
| `composer` | `v1/get_draft` | Request | `null` → `{recipient,subject,body}`, or null when closed | v0.3.0 |
| `composer` | `v1/set_draft` | Request | `{recipient?,subject?,body?}` | v0.3.0 |
| `composer` | `v1/opened` | Event | `{}` | v0.3.0 |
| `composer` | `v1/sending` | Event | `{}` (informational) | v0.3.0 |
| `reader` | `v1/get_html` | Request | `{message_id}` | v0.3.0 |
| `reader` | `v1/set_html` | Request | `{message_id,html}` | v0.3.0 |
| `reader` | `v1/add_banner` | Request | `{message_id,text,tone:"info"\|"warning"}` | v0.3.0 |
| `reader` | `v1/message_opened` | Event | `{message_id,subject,from,html,text}` | v0.3.0 |
| `inbox` | `v1/selected` | Event | `{message_id}` | v0.3.0 |
| `notifications` | `v1/notify` | Request | `{title,body}` | v0.3.0 |

Mail shell, accounts and AI have no bus surface.

## Behaviour

Composer buttons belong to their caller (four per caller, labels at most 32
characters, rendered as text). Clicking requests the caller's named operation with
`{draft:{recipient,subject,body}}`; returned `{draft:{...}}` fields update the editor.
Draft requests return null when the composer is closed. Reader awaits event delivery
before displaying the body. HTML replacements stay in its sandboxed iframe and show
“<extension name> changed how this message is shown”; banners are text only (at most
280 characters, the latest three kept). Notifications label the calling extension.
