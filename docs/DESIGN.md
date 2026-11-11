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
  designs. Mail that paints its own backgrounds or text colours (`bgcolor`, a CSS
  background, `color`) keeps its own light sheet inside the reader, so dark ink stays
  readable. Plain HTML mail is shown in the dark reading
  colours. Users can always switch to plain text.

## Layout

A slim top bar, then the workspace, flush to the window edges. Nothing floats on the
ground: panes meet at 1px separators, like the panels of a game engine's editor.

- **Top bar**: brand on the left, the command palette trigger (`Ctrl K`) centred, and
  Settings on the right. These are host controls and do not move. Compose joins them
  only when the Mail pane is hidden.
- **Workspace**: a tree. Splits divide space into rows or columns; every leaf is a
  group of tabbed panes with a 34 px tab strip. The default is Mail above the account
  switcher in a left column, with the Reader beside them.
- **Panes**: Mail (search with Compose beside it, folders, labels, list), Reader
  (message tabs, reader and composer), Accounts (the switcher) and one pane per enabled
  third-party panel.

Drag a pane's tab and a mint overlay shows where it will land: the tab strip or the
middle of a group adds a tab, the outer quarter of a group splits it on that side, and
the window's outer 14 px docks along that whole edge. Splitting keeps dividing, so a
column can hold three panes and the main area can become a 2 × 2 grid. The tab strip's
menu and `Layout:` palette commands do the same from the keyboard. Drag a separator to
resize, or focus it and use the arrow keys (Shift for bigger steps); double-click
evens out the two sides. Hide or show panes from the menu, the palette or
Settings → Extensions. The layout is an owner preference in the Host's database, so
it follows you between devices. Panes stay mounted while they move or sit behind
another tab, so rearranging never loses a draft.

Settings open as a card over the workspace, which stays mounted (and inert) behind a
dimmed backdrop, so an open message or draft is still there when Settings close. The
card has a section list, a breadcrumb and a close button; Escape or a click on the
backdrop closes it. On phones the card fills the screen. Under 800 px the app shows one
pane at a time, and folders become a bottom tab bar.

Reordering groups that share a row or column keeps every group's size; only a drop that
creates a new split divides space in half.

Every pane must work tall, wide and in between. The mailbox, reader, composer and
settings content size themselves with container queries (`mailbox`, `detail`,
`settings`, `pane`), never the window width: a mailbox in a wide group switches to
single-line rows, and a reader in a narrow one switches its actions to icons. Only
the shell itself uses media queries.

## Surfaces

Not everything deserves a card. Settings groups are sections divided by hairlines,
extensions and keys are rows, and notes sit directly on the page. Rounded, raised
surfaces are kept for things that float (menus, the palette, dialogs) and for small
controls. If a container needs a border, a background and a radius to make sense,
the content probably needs better spacing instead.

## Colour

Near-black and neutral, with just a breath of the brand's green; the mint accent carries the colour. Each colour has one job.

| Token | Use |
| --- | --- |
| `--ground` | Top bar and the window ground |
| `--bg` | Panes and Settings |
| `--sunk` | Inputs and inset notes |
| `--raise`, `--control` | Hover, active tabs, secondary buttons |
| `--select` | Selected rows, active chips, notices |
| `--line`, `--line-soft` | Hairlines; one elevation per element, border or shadow |
| `--ink`, `--ink-2`, `--ink-3` | Primary, secondary and tertiary text (all at least 4.5:1) |
| `--mint`, `--mint-hi` | The accent (brand mint by default): primary actions, focus, selection node |
| `--danger`, `--danger-bg` | Errors only |

The accent is the one colour the owner may change (Settings → Appearance). `--mint-hi`,
`--mint-ink` and `--select` derive from it with `color-mix`, so a new accent recolours
selection, notices and buttons consistently. Presets are light, low-chroma tones that read
both as text and as a fill on the dark ground.

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
The same mint marks the active pane tab's icon and underline, a connected account's
status dot (its glow is never clipped) and the drop preview while docking. The welcome
mark sits plainly in the empty reader, without a glow. Empty states may be a little warm (a time-of-day
greeting, a dry line when there are no API keys) but stay short. If unread state is added
later, show it with weight, not another dot.

## Motion

Follows Emil Kowalski's rules. Presses scale to 0.97. Listboxes grow from their
trigger in about 200 ms with a strong ease-out (`--ease-out`), a 2 px blur that clears
as they arrive, and leave in 120 ms back toward the trigger; the chosen option's check
pops in. Typing a letter jumps to a matching option. The Settings card rises 6 px and
scales from 0.97 in 220 ms. Notifications enter from the edge they are docked to,
leave the same way, and the stack slides (FLIP) when one goes. The command palette opens
instantly, because it is used hundreds of times a day. Docking previews slide
between zones in 160 ms; resizing and tab switches do not animate. The palette closes
on Escape or a click outside it. HTML mail fades
in once its frame has painted, after a skeleton, instead of flashing an empty sheet. List and folder changes do
not animate. `prefers-reduced-motion` removes every transition.

## Feels native

Colecto is an app window, not a page. The document never scrolls; every scroll area ends
at its content (a browser test checks each pane and Settings section). Interface text is
not selectable: only message text, inputs, identifiers in `code`, errors and anything
marked `.selectable` are. Images, icons and links cannot be dragged around the window.
Copy buttons sit in the reader toolbar and the composer footer.

## Notifications

In-app notifications cover new mail and automation waiting for send approval. Each has
a close button; approvals carry Approve, Reject and Review and stay until acted on, while
others leave after six seconds unless hovered or focused. Owners can turn them off
entirely or by type, choose one of eight positions on a drawn window (corners and edge
midpoints, never a screenshot), and opt into desktop notifications, which only fire while
Colecto is in the background. Notifications sit below Settings. The default is top center.

## Controls

No native `<select>`. Use `Select.svelte` (an accessible listbox) for choices with
more than a few options, and a segmented control for two or three. Checkboxes,
switches, file pickers (`FilePick.svelte`: a labelled drop zone over the native
input) and scrollbars are themed from the palette. Icons come from
`Icon.svelte` (Lucide paths, one stroke weight).
