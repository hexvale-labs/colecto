# Client usability and motion review

The shell now exposes Ctrl/Cmd+K commands for compose/save/send, folder navigation,
search, refresh, account switching, open tabs, extensions and settings.
Configuration and credential changes still use their existing forms. Keyboard
selection and dialog opening have no animation. The modal traps focus using the
native dialog and restores the prior connected element on dismissal.

## Adversarial data

`bun --cwd ui run vite --host 127.0.0.1` serves a development-only `/__stress`
route. Its Demo data / Worst case / Empty / 100 rows picker persists in the URL.
The fixture enters through Mailbox props and Reader's service boundary. Production
builds remove the route and its fixture import. The 100-row case matches the
current provider query cap; full-history pagination is not implemented.

| Surface | Source and limits | Cases |
| --- | --- | --- |
| Sender, address | Message strings; no separate UI limit | Long Polish name, plus-address email, empty sender, emoji |
| Subject, preview, body | Provider Message strings; draft subject 998 bytes, body 100000 bytes | Long subject/URL, CJK/Arabic, empty subject/body |
| Date | Message string, no separate UI limit | Long localized timestamp, empty date |
| Account name | AccountSettings; 100 bytes | Long profile text in reader toolbar |
| Labels | Rust metadata; 80 bytes/name, 50/account | Long multilingual label |
| Count/list | Current query capped at 100 messages | Zero, one, 100 rows |
| Avatar | Derived from sender/account | Full emoji grapheme and missing-name fallback |

| Finding | Change | Reason |
| --- | --- | --- |
| Reader used `charAt(0)` | Shared `Intl.Segmenter` helper | Preserve emoji and combining sequences |
| Timestamp had no width cap | Cap and ellipsize list dates | Keep sender/subject space available |
| Reader account badge could consume toolbar | Cap and ellipsize badge | Keep Back action reachable |
| Search excluded drafts and gave generic empty state | Search local draft fields; distinguish no matches | Make filtering predictable |

Browser regression cases exercise 320px, 390px and 1280px, enlarged root text,
long values, empty subjects, and emoji initials. Enlarged root text is not a
substitute for physical-device zoom testing. Native/physical devices remain
unchecked.

## Animation review

| Before | After | Why |
| --- | --- | --- |
| No explicit transitions or keyframes | Retained instantaneous mail/command actions | High-frequency keyboard actions should have no motion |
| Fine-pointer hover gating and reduced-motion scroll handling | Retained | No unsupported touch hover or forced motion added |

Verdict: approve the source motion behavior. There is no new animation requiring
frame-by-frame tuning or GPU promotion. Real-device responsiveness still needs
hardware validation. No animation library or dependency was added.
