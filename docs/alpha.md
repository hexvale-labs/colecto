# Alpha roadmap

Alpha means one owner can use Colecto as their daily client for real IMAP/SMTP
accounts, without data loss, and without opening another client for routine
mail, over IMAP/SMTP or JMAP. Desktop and browser come first; Android and iOS
builds exist but are lightly tested. Encryption of mail at rest, conversation threading, multi-device sync and
multi-user hosting come after alpha. Until alpha, persisted formats may change
without migrations.

| Task | Scope | Status |
| --- | --- | --- |
| 1. Messages | Reply, reply all, forward; sandboxed HTML reading; attachments in both directions | Done |
| 2. Mailbox | All server folders, paging and incremental refresh, read/unread, star, archive, delete, move | Next |
| 3. Alpha cut | Host state in `colecto.db`, drop legacy JSON imports, release builds, remove prototype wording | Planned |

## Reading and sending (task 1)

The list and offline snapshots still hold decoded text only. Opening a message
calls `GET /messages/{id}/content` (`messages.read`), which fetches the raw
message from the provider without marking it seen and returns full text, HTML,
attachment metadata, and `Message-ID`/`References`/`Reply-To`/`To`/`Cc`.
Messages above 30 MB are refused. Messages above 2 MB are listed from headers
and only load when opened. If the provider is unreachable the reader falls back
to the saved text, and replies are sent without threading headers.

HTML is untrusted. The reader renders it in an `<iframe sandbox>` with
`srcdoc`: no scripts, forms, plugins or same-origin access, and links open in a
new tab. The frame's own policy blocks remote images until the owner chooses
**Load remote images**; the Host policy allows inline styles and `https:` images
only so that frame can render. `cid:` inline images are not shown yet.

`GET /messages/{id}/attachments/{index}` returns one attachment as
`application/octet-stream` with an attachment disposition, so it can never
render in the app origin.

Sending accepts comma-separated `recipient` and `cc`, `in_reply_to` and
`references`, base64 `attachments` (at most 20 files and 25 MB total; the
`/send` route accepts bodies up to 36 MB), and `forward_of`, which re-attaches
the original message's attachments on the server and also requires
`messages.read`. Drafts keep recipients, Cc, threading and `forward_of`, but not
uploaded files. Delivery requests from API keys cannot include attachments or
forwards, because owner approval shows text only. JMAP sends threading headers,
multiple recipients and attachments.
