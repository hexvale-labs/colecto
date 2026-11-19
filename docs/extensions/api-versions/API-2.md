# API version 2

## Additions

- `section` on a `[[settings]]` table: `"appearance"` or `"notifications"`. The setting
  then appears in that core Settings section, under a heading with the extension's
  Settings title (or name), instead of on the extension's own page. Its key, value,
  `extension.settings` event and `settings.read` are unchanged. A manifest that uses
  `section` with `api_version = 1` is rejected:
  `settings.section needs api_version = 2 or later`.
- The icon `sparkles`, for AI-assisted actions.
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
