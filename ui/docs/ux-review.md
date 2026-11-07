# UI and mobile review

Reviewed with the installed `emil-design-eng` and `mobile-native` skills.

| Before | After | Why |
| --- | --- | --- |
| Account selector, account management and extension buttons all competed in the sidebar. | One profile row contains initial avatar, account name/account switcher, and a 44 px Settings control. | Keeps routine navigation focused while retaining keyboard-accessible account switching. |
| Account and extension configuration replaced the entire workspace through separate routes; clean extension navigation always asked for confirmation. | One Settings pane groups Accounts, Appearance, Servers and Extensions. Only dirty editors/settings trigger discard confirmation. | Reduces interruption and keeps configuration discoverable. |
| Only a light theme; native browser controls had no theme choice. | Dark is default; Light and System are persisted per device. System changes apply live and browser chrome follows the resolved theme. | Supports preference without a bright startup screen or mismatched controls. |
| Reading another message lost its place. | Account-scoped, closable message tabs; vertical scrolling on desktop and horizontal scrolling on narrow screens, with explicit 30-tab limit. | Preserves reading context without unbounded client memory or cross-account leakage. |
| Mobile account controls consumed multiple rows; long message titles could crowd navigation. | Compact profile row, bounded scrollable tabs, truncation with full accessible names, and independent folder navigation. Mobile Settings hides redundant mailbox navigation while retaining the account switcher. | Keeps all controls reachable without horizontal page overflow. |
| Settings and message readers could not show a provider Sent folder or search. | Inbox/Sent search requests use provider-backed service query contracts; Sent is an actual folder rather than a disabled placeholder. | Reuses provider semantics and reports real errors. |
| Remote API authentication failure could destroy the mounted editor. | A native modal requests the daemon token while retaining the editor behind it; reconnecting preserves text. | Prevents losing unsaved mail during authentication recovery. |
| Dark-mode error colors, browser form controls and account labels had no defined behavior. | Theme tokens cover surface/text/selection/error states; names fall back to email; native controls follow color-scheme. | Maintains legibility and handles optional names consistently. |

The existing safe-area viewport, dynamic viewport height, 16 px inputs, hover-capability media queries, selectable mail text, reduced-motion restraint, and overscroll containment are preserved. No decorative animation was added to frequent mail/navigation actions.

## Validation and limits

Behavioral Playwright coverage includes theme defaults/persistence/System changes, account-scoped tabs, narrow-screen reading/back navigation and horizontal overflow, unified settings without spurious confirmation, credential/account flows, unsaved draft handling, provider failures, extension enable/grant recovery, and session-token reconnect preserving draft text. Svelte and TypeScript checks and production build are required alongside these tests.

Physical Android hardware has not been available. Keyboard resize behavior, actual tap feel, accessibility service navigation, OEM background restrictions, and native FCM/UnifiedPush registration still require device validation. This browser client does not claim to register either Android push transport.

Remote server selection opens that daemon's same-origin UI; simultaneous aggregation of multiple service origins is not implemented. Tokens stay in sessionStorage (or memory when storage is unavailable), never localStorage or URLs. Mail bodies are not persisted as browser tabs.

Verified on this branch: `bun run --bun check` (0 errors, 0 warnings), `bun run --bun build`, and `bun run --bun test` (17/17 passing). A separate Chromium inspection measured document width 390 px at a 390 px viewport and checked dark mobile reader/settings renders. These remain browser checks, not physical-device validation.

The independent integration review also checked optional-name persistence: blank names remain blank in stored account settings, while account switchers, action labels and disconnect confirmation fall back to the current address. This keeps long addresses editable and avoids ambiguous account actions.
