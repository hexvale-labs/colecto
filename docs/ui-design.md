# UI & Navigation Specification

Colecto is **mobile-first in navigation clarity** and **platform-native in visual personality**.

## Adaptive 3-Tier Layout

Drop dockable/split game-engine panes. Use a proven adaptive mail layout:

| Viewport | Layout | Navigation |
| --- | --- | --- |
| **Phone** (`< 600px`) | Single-column navigation stack | Bottom nav or drawer (Inbox / Folders / Search) → Thread List (swipe actions, pull-to-refresh, compose FAB/button) → Pushed Thread Reader (native back gesture) → Compose sheet |
| **Tablet / Compact Desktop** (`600–1050px`) | Collapsible rail + master-detail split | Rail for accounts/folders, list + reader side-by-side or stacked on narrow panes |
| **Desktop / Wide Web** (`> 1050px`) | Classic 3-pane | Accounts & Folders sidebar \| Thread List \| Reading Pane, plus `Cmd/Ctrl+K` command palette |

## Per-Platform Design Systems

Share the controller, `MailBackend`, layouts and mail views, and branch only in the **Colecto kit** (`app/lib/ui/core/kit/`) and the navigation shell.

| Design (`COLECTO_DESIGN`) | Library | Native on | Signature details |
| --- | --- | --- | --- |
| `colecto` | `shadcn_ui` + Colecto palette | Linux, Web | Colecto top bar with centred command trigger, Compose beside search, glowing mint selection node, Mona Sans |
| `fluent` | `fluent_ui` | Windows | 48 px title bar (small icon + name, Fluent search box), *New mail* atop a Mica-ground navigation pane, accent selection pills, sender pictures in rows, labelled command bar, Segoe UI Variable, 4 px radii |
| `macos` | `macos_ui` | macOS | 52 px unified toolbar with room for traffic lights, mailbox title and count over the list, compose and search on the right, accent-tinted source list, accent-filled selected row and unread dot, icon-only toolbar, Settings as an icon-tab window |
| `material` | `material_ui` + `dynamic_color` | Android | Pill search bar, Compose FAB atop a navigation drawer with pill indicators, sender pictures in rows; on phones a large app bar, `NavigationBar`, extended FAB and bottom sheets; Material You colours |
| `cupertino` | `cupertino_ui` | iOS | iPadOS sidebar (large *Mailboxes* title, accent-filled selected row); on phones a large-title navigation bar, tab bar, action sheets and edge-swipe back |

### The Colecto kit

Feature code never imports a design library. Widget tests render every design at desktop, tablet and phone widths. It uses kit components, each with one API and one mapping per design:

| Component | Colecto (shadcn) | Fluent | macOS | Material 3 | Cupertino |
| --- | --- | --- | --- | --- | --- |
| `ColectoButton` | `ShadButton` variants | `FilledButton` / `Button` / `HyperlinkButton` | `PushButton` | `FilledButton` / tonal / `TextButton` | `CupertinoButton.filled` / `.tinted` |
| `ColectoIconButton` | `ShadIconButton.ghost` | `IconButton` | `MacosIconButton` | `IconButton` | `CupertinoButton` |
| `ColectoSwitch` | `ShadSwitch` | `ToggleSwitch` | `MacosSwitch` | `Switch` | `CupertinoSwitch` |
| `ColectoCheckbox` | `ShadCheckbox` | `Checkbox` | `MacosCheckbox` | `Checkbox` | `CupertinoCheckbox` |
| `ColectoTextField` | `ShadInput` | `TextBox` | `MacosTextField` | outlined `TextField` | `CupertinoTextField` |
| `ColectoSegmented` | Colecto segmented | `ToggleButton` row | `MacosSegmentedControl` | `SegmentedButton` | `CupertinoSlidingSegmentedControl` |
| `ColectoSelect` | `ShadSelect` | `ComboBox` | `MacosPopupButton` | `DropdownMenu` | action sheet |
| `showColectoConfirm` / `showColectoPrompt` | `ShadDialog.alert` | `ContentDialog` | `MacosAlertDialog` | `AlertDialog` | `CupertinoAlertDialog` |
| `showColectoContextMenu` | popover from the pointer | Fluent-metric popover | macOS-metric popover | bottom sheet | action sheet |
| `ColectoText` / `ColectoParagraph` | role → Colecto ramp | Windows type ramp | macOS ramp | M3 type scale | iOS Dynamic Type sizes |

Shared Colecto components (`ColectoChip`, `ColectoAvatar`, `ColectoSection`, `ColectoSettingRow`, `ColectoNotice`, `ColectoSkeleton`, toasts) take only radii and metrics from the design.

### Brand

The mark is Colecto's (`app/assets/branding/logo-square.svg`), drawn natively by `ColectoMark` and exported to every platform's app icons and the web favicon/manifest.

### Stress data

`just run demo <device> <design> worst` (`--dart-define=COLECTO_DEMO=worst`, development only) replaces the demo mailbox with worst realistic data: long names and unbreakable addresses, one-letter names, missing subjects, Japanese and Arabic text, emoji, 999+ counts, long label names, a 27-message thread, 3.2 GB and empty attachments, and hundreds of rows. Widget tests run it through every design at desktop, tablet and phone widths.

### Choosing a design

Each platform starts with its own design, but **any design works on any platform and at any window size**: someone can run Material on Windows or Fluent on Linux. The choice is an owner preference (`Preferences.design`, stored by `colecto-service`), set in Settings → Appearance → Design; picking the platform's own design clears it. For previews, `just run demo <device> <design>` and `just build web <design>` pass `--dart-define=COLECTO_DESIGN=<design>`, which overrides the saved choice for that launch.

### Rules that hold on every desktop design

- No Material ink splashes or ripples, no floating action buttons.
- Readable text (subjects, headers, bodies, addresses) is selectable.
- Right-click context menus on folders, labels, threads and messages.
- Desktop scroll behaviour without mobile overscroll; crisp scrollbars.
- Sharp 2px keyboard focus rings.
- Full keyboard navigation: `j`/`k` (or arrows) move, `e` archive, `#`/Delete trash, `s` star, `u` unread, `r` reply, `a` reply all, `f` forward, `c` compose, `/` search, `Esc` back out, `Cmd/Ctrl+K` command palette, `Cmd/Ctrl+,` settings. Every shortcut also has a pointer path.
- Light and dark mode both supported; the default follows the system.
- `Cmd/Ctrl +`, `-` and `0` zoom the whole interface like a browser (50–200 %, saved as `Preferences.zoom`); on touch designs the same setting is *Interface size* in Settings → Appearance.
- Send approvals appear (sidebar, Settings, command palette) only while a request is waiting.

### Android

Material You is on by default: with no accent chosen, `dynamic_color` supplies the whole palette from the wallpaper. Settings → Appearance → *Use system colours* turns it off and shows the accent picker. Swipe to archive or trash, pull to refresh, predictive back, long-press for actions.

### iOS

Large-title list, tab bar for Inbox/Drafts/Sent/Archive/More, edge-swipe back from the reader, Cupertino alerts and action sheets, swipe actions and pull to refresh.

## Layout and behaviour carried over from Colecto

- **Top bar** (desktop designs): brand, a centred `Search mail or run a command` trigger with its shortcut, account switcher with live sync status, Settings.
- **Mail pane:** search with Compose beside it, label filter chips with an inline *New label* field, sticky date groups, rows that become single lines when the pane is wide, hover actions (archive, read/unread, trash), Cmd/Ctrl-click and Shift-click selection with a bulk bar, and a footer with the conversation count and sync status. The list/reader divider can be dragged, nudged with the arrow keys, and double-clicked to reset.
- **Reader:** a sticky translucent toolbar, a centred letter (large subject, sender row with a stable per-correspondent avatar hue, label toggles, attachments that save to disk, 68ch body), earlier messages folded to one line, AI summary and reply drafting, and a welcome state with a time-of-day greeting.
- **Composer:** opens in the reader (a full-screen page on phones) with From/To/Cc/Subject rows, a save-state line, real file attachments, Cmd/Ctrl+Enter to send and Cmd/Ctrl+S to save. Closing with unsaved changes keeps them as a draft.
- **Settings:** a card over the workspace (full screen on phones) with a section list, breadcrumb and close button; groups are hairline sections, not cards.
- **Notifications:** enter from the edge they are docked to, leave after six seconds unless hovered.
- **Motion:** presses scale to 0.97, menus grow from their trigger in ~200 ms, the Settings card rises 6 px in 220 ms, the command palette opens instantly, and reduced-motion turns transitions off.

## Visual Identity: Colecto Palette + Native Controls

Colecto carries forward **Colecto's calm, green-tinted dark surface palette and typography discipline**, paired with **native, crisp platform controls** (shadcn-style on Desktop/Web, Material 3 on Android, Cupertino on iOS) rather than Colecto's custom web-only controls.

### Signature Dark Palette (Default Dark Theme)
Surfaces use a subtle cool-green tint (`oklch` hue ~165) with crisp hairline borders and Colecto's signature mint accent (user-customizable via `#rrggbb` in `Preferences.accent`):

| Token | Value | Role |
| --- | --- | --- |
| `--ground` | `oklch(0.112 0.002 165)` | App canvas / deepest backdrop |
| `--sunk` | `oklch(0.125 0.003 165)` | Recessed containers, search inputs |
| `--bg` | `oklch(0.148 0.003 165)` | Primary pane background |
| `--float` | `oklch(0.185 0.004 165)` | Popovers, menus, dialogs |
| `--raise` | `oklch(0.207 0.004 165)` | Elevated cards / hovered rows |
| `--control` | `oklch(0.235 0.005 165)` | Buttons, inputs, segmented controls |
| `--line` | `oklch(0.265 0.005 165)` | Primary 1px hairline dividers |
| `--line-soft` | `oklch(0.212 0.004 165)` | Subtle row/list separators |
| `--ink` | `oklch(0.95 0.003 155)` | Primary text |
| `--ink-2` | `oklch(0.79 0.006 155)` | Secondary text (snippets, metadata) |
| `--ink-3` | `oklch(0.63 0.008 158)` | Muted labels, timestamps |
| `--mint` | `#a2d6b1` | Default accent (selection, focus ring, unread dot, primary action) |
| `--select` | `color-mix(in oklch, var(--mint) 13%, var(--bg))` | Selected row / active nav item |
| `--status-online` | `#82c995` | Connected / push-active indicator |
| `--danger` / `--danger-bg` | `oklch(0.8 0.1 30)` / `oklch(0.245 0.045 28)` | Destructive actions and errors |
| `--star` | `oklch(0.85 0.12 85)` | Flagged/starred indicator |

Light mode uses matching neutral-mint surface counterparts (crisp white/off-white surfaces, deeper forest-mint accent for WCAG AA contrast) while keeping the same visual restraint.

### Typography, Density, and Accessibility
- **Typefaces:** Bundled `Mona Sans` variable font (OFL) on Colecto, Material and Cupertino; Segoe UI Variable on Fluent and the system font on macOS unless the owner picks one; with owner-selectable `Inter`, `Geist`, `IBM Plex Sans`, `Atkinson Hyperlegible Next`, or an installed system font (`Preferences.font`).
- **Base scale:** 13–18 px (`Preferences.font_size`, 14 px default).
- **WCAG 1.4.12 Text Spacing:** Independent 0–2 step controls for letter, word, line, and paragraph spacing (`Preferences.text_spacing`).
- **List density & grouping:** `full` (two-line with preview), `medium`, and `compact` (single-line) row densities (`Preferences.list_density`), plus optional relative date group headers (`Preferences.date_groups`).
- **Balance with native controls:** Use the palette and spacing tokens above, while letting each platform supply native scroll physics, focus/hover semantics, context menus, sheets, switches, and navigation chrome.
