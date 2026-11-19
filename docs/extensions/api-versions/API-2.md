# API version 2

## Additions

- `section` on a `[[settings]]` table: `"appearance"` or `"notifications"`. The setting
  then appears in that core Settings section, under a heading with the extension's
  Settings title (or name), instead of on the extension's own page. Its key, value,
  `extension.settings` event and `settings.read` are unchanged. A manifest that uses
  `section` with `api_version = 1` is rejected:
  `settings.section needs api_version = 2 or later`.
- The icon `sparkles`, for AI-assisted actions.
- Row context menus. A `list` with `context_menu: true` sends
  `event {kind: "context", value: {key}}` when a row is right-clicked or gets Shift+F10 or
  the Menu key. The owner answers with
  `{t: "menu", surface, node, key, items: [{id, label, icon?, danger?, disabled?, checked?, separator?}]}`
  (at most 24 items, an empty list opens nothing); the shell draws the menu at the pointer
  or under the row and sends `event {kind: "menu", value: {key, item}}` for the choice. A
  `menu` that does not answer the latest request is ignored.
- `message_row` takes `starred` and shows a star.
- Messages carry `thread`, the provider's conversation id (JMAP `threadId`; for IMAP the
  first `References` id, else `In-Reply-To`, else the `Message-ID`). It may be empty.
- The icons `archive`, `eye`, `filter`, `folder`, `star` and `tag`.
- `menu_button` draws its `icon` before the label, and narrow readers show only the icon,
  like other tools. API 1 accepted the prop but drew nothing.

## Breaking changes

- N/A

## Notes

Sections that hold credentials, connections, keys or other extensions (Accounts,
Connection, API keys, Send approvals, Extensions) accept no extension settings. An
extension whose settings all name a section and that draws no `settings` surface gets
no page of its own. `Notifications` became a core section in this version, so it is
also reserved as a `settings_page.title`. See
[Extension settings](../README.md#extension-settings).
