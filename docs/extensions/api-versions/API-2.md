# API version 2

Mail lists reach every folder the provider has, and older mail one page at a time.

## Additions

- `POST /api/v1/messages/refresh` and `GET /api/v1/messages/cache` accept any folder id
  that `GET /api/v1/folders` lists as `folder` (an IMAP mailbox name or a JMAP mailbox
  id), besides `inbox` and `sent`. `inbox` and `sent` always mean those roles.
- `POST /api/v1/messages/refresh` accepts `offset` (0 to 100000): skip that many of the
  newest messages and return the next page of at most 100. An empty page means there is
  no older mail.
- The `list` component takes `more` (`available`, `loading`, `end`, `error`) and sends
  `end` when it is scrolled near its end while `more` is `available`. See
  [Collections](../ui-model.md#collections).

## Breaking changes

- N/A

## Notes

Extensions on API 1 that pass another folder or a non-zero `offset` get
`400 Other folders and offset need api_version = 2 or later`, the same refusal they
always got. Only the newest page of Inbox and Sent is saved as a snapshot, so
`/messages/cache` returns an empty list for other folders. See
[Cached folders and error categories](../README.md#cached-folders-and-error-categories).
