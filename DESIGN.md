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

A slim top bar, then docks of panes on a near-black ground.

- **Top bar**: brand, the command palette trigger (`Ctrl K`), Compose, the account
  switcher and Settings. These are host controls and do not move.
- **Docks**: left, center, right and bottom. Each pane is a rounded tile; several
  panes in one dock become tabs in the tile's header. The default is Mail on the
  left (344 px) and Reader in the center.
- **Panes**: Mail (search, folders, labels, list), Reader (message tabs, reader and
  composer) and one pane per enabled third-party extension panel.

Move a pane by dragging its tab onto a drop zone (a dashed mint preview shows where
it lands), with the layout button in the tile header, or with `Layout:` commands in
the palette. Drag the gutter between tiles to resize, or focus it and use the arrow
keys (Shift for bigger steps); double-click restores the default size. The layout
is remembered per browser in `localStorage` (`colecto.layout.v1`). Hidden panes stay
mounted, so switching tabs or moving a pane never loses a draft.

Settings replace the docks with one tile holding a section list and the section.
Under 800 px the app shows one pane at a time, and folders become a bottom tab bar.

Every pane must work tall, wide and in between. The mailbox, reader, composer and
settings content size themselves with container queries (`mailbox`, `detail`,
`settings`, `pane`), never the window width: a mailbox in a wide dock switches to
single-line rows, and a reader in a narrow one switches its actions to icons. Only
the shell itself uses media queries.

## Colour

Near-black and neutral, with just a breath of the brand's green; the mint accent carries the colour. Each colour has one job.

| Token | Use |
| --- | --- |
| `--ground` | Near-black app ground behind the tiles |
| `--bg` | Pane tiles |
| `--card` | Settings cards, a step above the tile |
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

The selected message carries a small, softly glowing mint dot, the same
incoming-message node as in the logo. It means "this is the one you are reading".
The same mint marks the active dock tab's icon, a connected account's status dot and
the drop preview while docking. Empty states may be a little warm (a time-of-day
greeting, a dry line when there are no API keys) but stay short. If unread state is added
later, show it with weight, not another dot.

## Motion

Follows Emil Kowalski's rules. Presses scale to 0.97. Listboxes grow from their
trigger in 160 ms with a strong ease-out (`--ease-out`). The command palette opens
instantly, because it is used hundreds of times a day. Docking previews slide
between zones in 160 ms; resizing and tab switches do not animate. HTML mail fades
in once its frame has painted, after a skeleton, instead of flashing an empty sheet. List and folder changes do
not animate. `prefers-reduced-motion` removes every transition.

## Controls

No native `<select>`. Use `Select.svelte` (an accessible listbox) for choices with
more than a few options, and a segmented control for two or three. Checkboxes,
switches, file pickers (`FilePick.svelte`: a labelled drop zone over the native
input) and scrollbars are themed from the palette. Icons come from
`Icon.svelte` (Lucide paths, one stroke weight).
