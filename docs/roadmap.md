# Roadmap

**Alpha** means one owner can use Colecto as their daily mail client over IMAP/SMTP or
JMAP, without data loss and without opening another client for routine mail. Desktop
and browser come first; Android builds exist but are lightly tested. Until the first
stable release, stored formats may change without migrations.

Statuses: **Done**, **In progress**, **Next**, **Later**.

## Mail

| Item | Status |
| --- | --- |
| Reply, reply all, forward; sandboxed HTML reading; attachments both ways | Done |
| IMAP IDLE and JMAP push, with polling fallback | Done |
| Discover every server folder with its role and counts (`GET /folders`) | Done |
| Read/unread and star flags; move, archive and delete | In progress |
| Browse any folder in the UI, with paging through full history | Next |
| Incremental refresh of cached folders instead of refetching | Next |
| Larger offline cache (thousands of messages per account) | Next |
| Conversation threading | Later |
| Inline `cid:` images in HTML mail | Later |
| Encryption of stored mail | Later |

## App

| Item | Status |
| --- | --- |
| Dockable workspace, command runner, notifications, accent colour | Done |
| Extensions: Wasm panels and overlays, install-time consent, z-index layers | Done |
| Declarative extension settings, owner controls and per-guest reads | Done |
| Onboarding for the first account; demo account off by default | Next |
| Font choice and size, account avatars and account settings dialog | Done |
| Loading screen and selection polish | Next |
| Custom ports in every server field | Next |
| Install the daemon as a background service (systemd user unit, Windows logon task) | Next |
| Stress testing with thousands of messages and very long HTML mail | Next |
| Warning when an extension targets an API version that can no longer fully work | Later |

## Platforms and hosting

| Item | Status |
| --- | --- |
| Linux, Windows, browser; Docker image `ghcr.io/hexvale-labs/colectod`; AUR packages | Done |
| Android: UnifiedPush from a self-hosted daemon (Web Push, no foreground service) | Next |
| Android: optional foreground service for local accounts without a daemon | Next |
| Signed macOS and iOS builds | Later (needs an Apple developer account) |
| Several users on one daemon | Later |
| Synchronizing state between devices | Later |

## Not planned

- POP3
- 32-bit platforms (Android `armv7` and `x86`, 32-bit Windows and Linux). Release CI
  builds 64-bit `x64` and `arm64` only.

## Android push design

When the app is connected to a daemon, the daemon already watches every account, so
the phone does not need its own connection:

1. The app registers with a UnifiedPush distributor (for example ntfy) and sends the
   resulting endpoint and Web Push keys to the daemon.
2. On new mail the daemon sends an encrypted Web Push message (RFC 8291, VAPID
   signed, at most 4 KB) with the account, count and newest senders and subjects.
3. The distributor wakes the app for a few seconds; a small Kotlin receiver shows the
   notification at once and queues a WorkManager job to refresh the cache.

Without a daemon, an optional `specialUse` foreground service keeps the Rust core's
IDLE/push watchers running, with a low-priority persistent notification and a request
to ignore battery optimisation. `dataSync` is unsuitable: Android 15 limits it to six
hours a day and blocks starting it at boot.
