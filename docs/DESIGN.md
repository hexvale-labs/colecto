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
ground: draw each boundary once; prefer spacing and tone over lines. Dock splitters
provide the single divider between adjacent panes. Pane headers and list footers
use spacing rather than additional rules.

- **Top bar**: brand on the left, the command palette trigger (`Ctrl K`) centred, and
  Settings on the right. These are host controls and do not move. Compose joins them
  only when the Mail pane is hidden.
  The palette's "Reload the app" stays available while the app is busy, because a reload
  is the way out when it is stuck; it asks first when there are unsaved changes.
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

Every setting has a name and a one-line description. A settings page reads in four steps:
the page title, a group label, the setting's name, then its muted description. Only those
first three carry weight; everything else is regular text. Shared classes in `style.css` give
every page the same rhythm, and new pages (built-in or extension) use them:

| Class | Use |
| --- | --- |
| `.section-head` | Page title (`h2`) and one-paragraph description |
| `.settings-group` | A group of settings, separated from the next by space; `.group-label` (an `h3` or a fieldset `legend`) names it, and an optional `p` after the label describes it |
| `.setting` | One setting; a wide control (picker, slider, drop zone) sits under its text |
| `.setting.setting-row` | Text on the left, the control (switch, segmented control) on the right; under 520 px a segmented control drops below the text |
| `.setting-text` | The setting's name (`h3`) and description (`p`) |
| `.settings-list` | A list of things (accounts, extensions, keys, requests); the only place hairlines appear |

`.settings-card` is the older name for `.settings-group`, still used by the Accounts and
Connection pages.

Settings controls follow the value's shape: a switch for on/off, a segmented control
for two to four choices, a list for more, and a slider (`.slider`, accent-filled up to
the thumb) for a bounded number, with the current value beside its label and the ends
underneath. Touch and pen input on a slider goes through `use:touchSlider`
(`ui/src/lib/slider.ts`), not the engine: the Android app's WebView never moved a styled
range under a finger. A drag follows the finger, a tap jumps on release, and
`touch-action: pan-y` leaves vertical swipes to scrolling without changing the value. On
coarse pointers the thumb grows to 22 px and the hit area to 40 px. Mouse input stays
native. Settings save as they change; there is no Save button. An extension's own
settings page (`ExtensionSettings.svelte`) renders its manifest's `[[settings]]` with
these same controls.

Settings open as a card over the workspace, which stays mounted (and inert) behind a
dimmed backdrop, so an open message or draft is still there when Settings close. The
card has a section list, a breadcrumb and a close button; Escape or a click on the
backdrop closes it. The section list is a sidebar that collapses to an icon rail (its
button sits beside the title); the choice is remembered per device in `localStorage`
(`colecto.settingsNavCollapsed`), and starts collapsed below 840 px. Below 840 px, and
on short touch screens such as a phone on its side, the card fills the screen. On a
compact window (below 600 px) Settings opens on the list of sections, full width like a
phone's settings app, and each section opens over it with a back arrow; the page's own
heading names it.

## Adaptive layout

The shell follows Android's window size classes, by window width rather than device:

| Class | Width | Layout |
| --- | --- | --- |
| Compact | below 600 px | One pane at a time; folders in a bottom navigation bar; Settings as list, then page |
| Medium | 600 to 839 px | One pane at a time; folders in an 80 px navigation rail on the leading edge |
| Expanded | 840 px and up | The dockable workspace described above |

The bar and the rail mark the current folder with Material's pill indicator, which widens
in from the middle. Window classes change while the app runs (rotation, split screen,
folding, resizing a ChromeOS window), and the layout follows. Only the shell reads them,
through media queries and `matchMedia` in `MailShell.svelte`.

## Back and gestures

`ui/src/lib/back.ts` keeps one back stack. Each thing you can go back from claims an
entry while it is open, and each entry is a browser history entry (`history.state`
`{ colectoBack: n }`), so the system back button, the browser's back, a mouse back button
and the in-app back arrows all close the newest thing first:

| Claims back | While |
| --- | --- |
| Every modal `<dialog>` | Open (`showModal` is wrapped once; back sends it `cancel`, like Escape) |
| An open listbox (`Select.svelte`) | Open |
| Settings | Open |
| A Settings page | Open over the section list (compact) |
| The reader, composer or a panel | Shown instead of the mail list (compact and medium) |

The owner can still refuse (unsaved changes): the entry then claims its place again. With
nothing claimed, back leaves the app (Android) or the page (browser). A reload starts a
fresh stack. In-app back arrows call `goBack()`, so they animate exactly like the system
back.

On Android the native plugin turns the predictive back gesture into `colecto:back` events
(`start`, `progress`, `cancel`); the screen on top shrinks toward 90 %, drifts away from the
edge, rounds its corners and shows what it will reveal, then leaves on commit. Android's
own back-to-home preview does not play yet: Tauri always registers its own back callback
(tauri-apps/tauri#16430 would make that optional). iOS has no back button, so a swipe from
the left 24 px follows the finger and goes back past 35 % of the width or on a fast flick.

## Press feedback

In the Android app every tap on a control (button, link, tab, option, switch) plays the
system click and toggles add a light tick, through the plugin's `press` command; Android
plays them only when the owner has touch sounds and touch feedback on. iOS controls do not
click, so iOS gets only the tick on toggles. Browsers and desktop get neither.

Reordering groups that share a row or column keeps every group's size; only a drop that
creates a new split divides space in half.

Every pane must work tall, wide and in between. The mailbox, reader, composer and
settings content size themselves with container queries (`mailbox`, `detail`,
`settings`, `pane`), never the window width: a mailbox in a wide group switches to
single-line rows, and a reader in a narrow one switches its actions to icons. Only
the shell itself uses media queries.

## Surfaces

Not everything deserves a card. Settings groups are separated by space, not lines;
extensions, keys and accounts are rows of a list with a hairline between them, and notes
sit directly on the page. Rounded, raised
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
106 to 108 %) rather than a second typeface. No all-caps eyebrow labels, with one
exception: the Settings section list groups its pages under small uppercase *Built-in*
and *Extensions* labels (written in normal case, uppercased with CSS so screen readers
read words). Monospace
is only for real identifiers such as extension capabilities.

## Signature

The selected message carries a small, softly glowing mint dot, the same
incoming-message node as in the logo. It means "this is the one you are reading".
The same mint marks the active pane tab's icon and underline and the drop preview
while docking. A connected account's status dot is the one exception: it is always
green (see below), whatever the accent. The welcome
mark sits plainly in the empty reader, without a glow. Empty states may be a little warm (a time-of-day
greeting, a dry line when there are no API keys) but stay short. If unread state is added
later, show it with weight, not another dot.

## Motion

Follows Emil Kowalski's rules. Presses scale to 0.97. Listboxes grow from their
trigger in about 200 ms with a strong ease-out (`--ease-out`), a 2 px blur that clears
as they arrive, and leave in 120 ms back toward the trigger; the chosen option's check
pops in. Typing a letter jumps to a matching option. The Settings card rises 6 px and
scales from 0.97 in 220 ms. Notifications enter from the edge they are docked to and
leave the same way, fading in place while the rest of the stack slides (FLIP) into the
space. Their transitions move only `translate`, `scale` and `opacity`: Svelte holds a
leaving toast in place with a `transform`, and an exit that animated `transform` would
throw it to the wrong spot for its last frames. On compact and medium windows a screen that replaces the mail list (a message, the
composer, a panel, a Settings page) slides in over it: on iOS fully from the right in
300 ms with the drawer curve (`cubic-bezier(0.32, 0.72, 0, 1)`), elsewhere as Material's
shared-axis step (96 px or 30 % of the width, with a fade) in 240 ms. Going back plays it
in reverse in about 200 ms, with the covered screen shown underneath. Gestures drive the
same surfaces (see "Back and gestures"). Collapsing the Settings section list animates
its column in 240 ms. The command palette opens
instantly, because it is used hundreds of times a day. Docking previews slide
between zones in 160 ms; resizing and tab switches do not animate. The palette closes
on Escape or a click outside it. HTML mail appears
as soon as its body arrives, after a skeleton; it never waits for the frame's `load`
event, which waits for every remote image and can hang on a slow server. List and folder changes do
not animate. `prefers-reduced-motion` removes every transition.

## Feels native

Colecto is an app window, not a page. The document never scrolls; every scroll area ends
at its content (a browser test checks each pane and Settings section). Hover styles apply
only with `(hover: hover) and (pointer: fine)`, so a tap never leaves a highlight behind;
presses use `:active`. Keyboard hints (`Ctrl K`, `Esc`, the palette's key legend, the
welcome tip) are hidden where no fine pointer exists (`not all and (any-pointer: fine)`).
Inputs stay at least 16 px on compact and medium windows so iOS does not zoom into them. Interface text is
not selectable: only message text, inputs, identifiers in `code`, errors and anything
marked `.selectable` are. Images, icons and links cannot be dragged around the window.
Copy buttons sit in the reader toolbar and the composer footer.

## Notifications

In-app notifications cover new mail and automation waiting for send approval. Each has
a close button; approvals carry Approve, Reject and Review and stay until acted on, while
others leave after six seconds unless hovered or focused. Owners can turn them off
entirely or by type, choose one of eight positions on a drawn window (corners and edge
midpoints, never a screenshot), and opt into desktop notifications, which only fire while
Colecto is in the background. The default is top center. Action buttons start at the
toast's left edge, under the icon.

Notifications show above everything in the window, Settings and menus included: the region
is a `popover="manual"` element in the browser's top layer. A modal dialog opened later
(an account sheet, the install warning) goes above it, and the next notification raises the
region over the dialog again. Where popovers are missing, the overlay's layer (1250) still
keeps it above Settings.

**New updates** (on by default) tells the owner once per version when a newer stable
release is published, with a **View release** button that opens the release page (in the
desktop app, in the system browser). The UI asks the Host's `GET /api/v1/releases/latest`
once per start; turned off, it asks nothing. The version already announced is the owner
preference `notified_release`.

## Controls

No native `<select>`. Use `Select.svelte` (an accessible listbox) for choices with
more than a few options, and a segmented control for two or three. Checkboxes,
switches, file pickers (`FilePick.svelte`: a labelled drop zone over the native
input; `ondropfiles` lets a page check dropped files itself, and `dropping` highlights it
while a file is dragged anywhere over the page) and scrollbars are themed from the palette. Icons come from
`Icon.svelte` (Lucide paths, one stroke weight).

## Connection status and startup

`--status-online` is a calm green independent of the configurable accent. Connected
account dots use it with a subtle 2400 ms expanding ring (scale 1 to 2.4, opacity
0.5 to 0). Disconnected dots stay neutral. Reduced motion disables the ring.

Desktop message tabs share the dock header with the Reader tab; its drag handle,
arrange menu and pane semantics remain available. Compact mode keeps message tabs
inside the reader because it has no dock header. Tab strips scroll horizontally.

The static `index.html` startup splash (the full logo, 128 px wide and centred, on
`--ground`; `logo-full.svg`'s view box is trimmed to the artwork so it centres; the wordmark is
vector, so it needs no web font) paints before mounting, with a two-frame
paint checkpoint. It stays for at least 800 ms after that checkpoint and until a
usable app surface exists, then fades over 200 ms with ease-out without moving the
layout. Reduced motion removes the fade. Automated browsers can set sessionStorage
`colecto.fastStartup` to `1` to skip timing; normal browsers ignore that hook.

## Typography and settings controls

Appearance offers five bundled fonts: Mona Sans (the default, in the main bundle),
Inter, Geist, IBM Plex Sans and Atkinson Hyperlegible Next. The others load only
when chosen or when the font menu opens, so the default bundle does not grow. The menu
lists **Bundled** and **System** families, each previewed in its own face. Browsers
cannot list installed fonts without asking (WebKit cannot at all), so System shows the
common families `probeSystemFonts()` in `lib/typography.ts` finds by measuring text;
where the Local Font Access API exists, **List all installed fonts** asks for the full list.

Text size is four steps (13, 14, 16 and 18 px; 14 is the design size), shown as "Aa"
samples. On a touch screen (`pointer: coarse`) every step reads larger by 18/14, so the
default there is the desktop's largest size and the steps above it go further; the saved
preference is the same on every device. Every `font-size` in the stylesheets is written as
`calc(<px> * var(--text-scale))`, and the setting sets `--text-scale`. Write new font
sizes the same way. Spacing and pane geometry stay in px on purpose, so docking and
layout maths do not change with text size.

Owner preferences store `font` (a bundled id or a system family, at most 64
characters) and `font_size` (13 to 18).

## Message list

Settings → Appearance → Message list chooses the density and whether date headings show.

| Preference | Values | Default | Effect |
| --- | --- | --- | --- |
| `list_density` | `full`, `medium`, `compact` | `full` | Full: sender and date, subject, preview. Medium: no preview, tighter rows. Compact: one line (sender, subject, date); in a wide mailbox the preview stays on that line |
| `date_groups` | `true`, `false` | `true` | Headings such as Today, Yesterday and month names between rows (only when sorted by date) |

Density changes padding and lines, never type size. On touch layouts a compact row stays at
least 44 px tall.

Keep switches for booleans, radio segments for small exclusive choices, accent
swatches for colours and the corner picker for notification placement. Native
account dialogs trap focus, support Escape, scroll within the viewport and keep
destructive actions behind a confirmation.
