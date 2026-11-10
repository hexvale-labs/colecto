# Colecto design

The bundled UI is a working tool people open many times a day. It should feel calm,
dense enough to scan, and obvious to navigate. The brand shows up in small, exact
details rather than decoration. Tokens live in `ui/src/style.css`; brand files live
in `docs/branding/` (copied into `ui/src/assets/` for the build).

## Light mode

Colecto has no light mode. Not hidden, not "coming soon". It does not exist.

This was a deliberate decision, made with a straight face and a slightly raised
eyebrow. It is 2026. We polled everyone we know and could not find a single person
who reads mail on a white rectangle by choice. If you have met one, please do not
approach them suddenly. Observe them calmly from a distance and take notes. Light
mode users should be studied, ideally by a team of researchers with a grant,
sunglasses and a lot of patience.

In practical terms:

- There is no theme setting, no `prefers-color-scheme: light` branch, and no
  `data-theme` attribute. `color-scheme: dark` is set once on `:root` and in
  `index.html`.
- Do not add light tokens "just in case". A pull request that adds a light theme
  will be lovingly closed and its author enrolled in the study above.
- HTML mail is the only exception, and only out of respect for other people's
  designs. Mail that paints its own backgrounds (`bgcolor` or a CSS background)
  keeps its own sheet inside the reader. Plain HTML mail is shown in the dark reading
  colours. Users can always switch to plain text.

## Layout

Two columns, never three.

- **Rail** (left, 344 px, 304 px under 1100 px): brand, command palette and
  Compose; search; folders; label chips; the message list; account switcher and
  Settings at the bottom.
- **Stage** (right): open-message tabs, then the reader, the composer, an extension
  panel, or settings.

Settings replace the rail's contents with a section list and use the stage for the
section, so there is still no third column. Under 800 px the app shows one column
at a time, and folders become a bottom tab bar.

The mailbox, reader, composer and settings content are widgets that size
themselves with container queries (`mailbox`, `detail`, `settings`), not with the
window width. A mailbox placed in a wide slot switches to single-line rows; a
reader in a narrow slot switches its actions to icons. Keep new widget styles on
container queries so they still work once widgets can be rearranged. Only the
shell itself uses media queries.

## Colour

Restrained, tinted from the brand's dark green-grey. Each colour has one job.

| Token | Use |
| --- | --- |
| `--bg` | Stage ground |
| `--rail` | Rail ground, slightly lifted and greener than the stage |
| `--sunk` | Inputs and inset notes |
| `--raise`, `--control` | Hover, active tabs, secondary buttons |
| `--select` | Selected rows, active chips, notices |
| `--line`, `--line-soft` | Hairlines; one elevation per element, border or shadow |
| `--ink`, `--ink-2`, `--ink-3` | Primary, secondary and tertiary text (all at least 4.5:1) |
| `--mint`, `--mint-hi` | Brand mint: primary actions, focus, selection node |
| `--danger`, `--danger-bg` | Errors only |

Sender avatars take a stable hue from the address (`hue()` in `lib/initial.ts`), so
the same correspondent is recognisable at a glance.

## Type

Mona Sans (variable weight and width, bundled through Fontsource so the desktop app
works offline). UI text is 13 to 14 px; message bodies are 15.5 px with 1.68 line
height and a 68ch measure. Headings use a slightly wider width (`font-stretch`
106 to 108 %) rather than a second typeface. No all-caps eyebrow labels. Monospace
is only for real identifiers such as extension capabilities.

## Signature

The selected message carries a small mint dot, the same incoming-message node as in
the logo. It means "this is the one you are reading". If unread state is added
later, show it with weight, not another dot.

## Motion

Follows Emil Kowalski's rules. Presses scale to 0.97. Listboxes grow from their
trigger in 160 ms with a strong ease-out (`--ease-out`). The command palette opens
instantly, because it is used hundreds of times a day. List and folder changes do
not animate. `prefers-reduced-motion` removes every transition.

## Controls

No native `<select>`. Use `Select.svelte` (an accessible listbox) for choices with
more than a few options, and a segmented control for two or three. Checkboxes,
switches, file pickers and scrollbars are themed from the palette. Icons come from
`Icon.svelte` (Lucide paths, one stroke weight).
