# Mail client, scoped automation, and resource efficiency

Status: proposed for final design approval. Product direction confirmed in chat;
implementation choices below are proposals unless explicitly identified otherwise.

## Intent and agreed requirements

Make Colecto a practical email client with HTML reading, a fuller composer,
local Turso caching, readable failures, encryption, verified company indicators,
and MCP access from the desktop/local Host and daemon. Research native Tauri CPU
and memory use, measure changes, and keep optimizations only when their benefit
justifies their complexity.

Each operating-system user runs their own daemon as a user service, with their
own data, mail accounts, credentials, and generated API keys. There is no shared
multi-user daemon, application signup, administrator, or tenant database.
MCP may send email only after human approval. Timeout is not approval.

## Approaches

1. Extend the shared Rust MailService with scoped principals, automation approval,
   and MCP adapters. Use embedded Turso locally. This preserves the current
   deployment and extension boundaries; recommended and consistent with the
   owner's confirmed direction.
2. Build separate authentication and mail implementations in the desktop and
   daemon. This duplicates policy and risks different permission enforcement;
   rejected in favor of sharing the service layer.
3. Add a hosted multi-user service and cloud database. This requires user login,
   tenancy isolation, and deployment work unrelated to the requested per-user
   daemon; explicitly excluded by the owner.

## Current boundaries

Core services own accounts, secrets, mail, drafts, and delivery. Host owns
authentication, extension grants, recovery, and platform adapters. Trusted bundled
Svelte extensions provide mailbox, reader, composer, and shell; third-party Wasm
extensions contribute bounded text-and-action panels. They never receive database
or native IPC access. Keep these boundaries.

Remote access currently uses one owner bearer token; loopback access is local.
Extension/account HTTP headers route trusted UI calls and are not authenticated
identities. New API key callers must never acquire trusted UI privileges by
setting those headers. Tauri and the daemon use one MailService implementation.

## Authentication and API keys

Add an authenticated principal at the service boundary: trusted owner session or
scoped API key. Bind keys to the current installation's OS-user owner, not a
client-supplied user ID. The existing owner credential remains the remote
bootstrap/recovery credential; scoped keys cannot create owner sessions.

Settings permits the owner to generate named cryptographically random keys,
select explicit account IDs and existing operation capabilities, choose optional
expiry, inspect creation/last-used metadata, and revoke keys. Show the full secret
only once; persist a public lookup identifier and cryptographic verifier, never
the secret. Never put secrets in URLs, logs, mail responses, or extension output.
Use private storage permissions and bound the number of keys and metadata sizes.

Share capability vocabulary and operation checks with extension authorization,
but keep principal authentication and extension grants distinct. API keys have
no implicit extension identity, account wildcard, credential access, vault control,
extension-policy access, key-management access, or approval access. New accounts
are not automatically granted. Reject expired/revoked keys on every request,
including calls within an existing MCP connection. Enforce the same policy on
existing HTTP mail routes so they cannot bypass MCP restrictions. Missing or
corrupt key policy fails closed; it never restores broad access.

Owner controls must be inaccessible to scoped keys even on loopback. An
unauthenticated browser origin must not be able to mint keys or approve mail;
retain Host/Origin validation and establish an owner session for privileged local
controls. Existing Wasm grants retain their documented behavior; this change does
not silently broaden or remove third-party extension permissions.

## MCP and human approval

Expose one tool implementation through shared service adapters for the local
client Host and daemon. Provide a local stdio adapter for MCP clients and an
authenticated HTTP endpoint for clients supporting configured bearer credentials.
Bind each session/call to its key and account grants; tool discovery does not
confer permission. Document the selected stable protocol version and actual
client compatibility. Static API key authentication is not a claim of complete
OAuth discovery or universal remote-MCP compatibility. Do not implement an OAuth
authorization server as part of this slice.

Tools cover permitted account connection metadata, message list/read, draft
list/read/write, labels, and requesting delivery. Tool responses exclude secrets.
All external MCP/API-key sends create a durable pending request containing an
immutable, validated message snapshot and the requesting key/account identity.
Return a request ID and status promptly; do not depend on holding a network
request open throughout human review. The requester may inspect only its own
requests. Bound pending requests, request sizes, and polling frequency.

The owner reviews From, To/Cc/Bcc, subject, body, attachments, and encryption state
in Colecto. Approving sends that exact snapshot once; edits require a new request.
Default pending expiry is 10 minutes. Reject, cancel, expiry, key revocation,
removed account, or missing grants prevents delivery. Recheck policy before the
send starts. Elicitation may notify clients that support it, but an agent's own
tool response cannot act as the owner's approval.

Persist state transitions before delivery. A transport failure or process crash
after submission may have an uncertain outcome; mark it accordingly and never
retry automatically. Replayed approvals cannot trigger a second submission.
This is at-most-once application initiation, not a guarantee of exactly-once
SMTP delivery. Purge retained request bodies after a bounded review period.

## Turso cache and fetching

Use the embedded Rust Turso engine and local files, without requiring a Turso
cloud account or uploading mail. Keep stores account/provider-identity scoped.
Preserve the initial inbox bounds of 100 messages and 12 MB serialized content;
add folder and fetch-time metadata rather than claiming complete offline sync.
Keep small bounded in-memory reader data, avoiding duplicate full mailbox copies.

Import existing valid mail-cache.json snapshots transactionally once and preserve
the source file for rollback. Do not move credentials, drafts, or extension grants
as incidental database cleanup. Version the schema; reject unsupported versions
without deleting data. Failed fetches/writes preserve the last successful cache.
Cached messages appear immediately, with last-fetched/stale state, an accessible
refresh indicator, and retry feedback. Disconnected readers retain cached mail.
The cache remains local plaintext; encrypted-message decrypted bodies must not
be added to persistent plaintext cache. Disk encryption is outside this slice.

## Reading and composing

Preserve separate text and HTML MIME alternatives across IMAP and JMAP. Sanitize
HTML with a maintained parser/allowlist and render in an isolated sandbox with
restrictive CSP. Remove scripts, forms, active embeds, event handlers, dangerous
URLs, and network-loading CSS. Block remote resources by default. Any explicit
remote-image loading must avoid turning the Host into an unrestricted URL proxy.
Keep a plain-text reading option and safe external-link handling. Mail HTML never
executes in the trusted extension or application origin.

Extend the composer with multiple recipients, Cc/Bcc, bounded attachments,
reply/reply-all/forward, and basic rich text with a plain-text alternative.
Preserve threading headers, omit Bcc from delivered headers, and support these
fields in drafts, validation, SMTP/JMAP delivery, and approval snapshots. Include
accessible formatting controls, attachment removal, dirty-state protection, and
clear save/send feedback. No offline send queue or automatic resend is implied.

## Encryption and verified senders

Use maintained OpenPGP/MIME and S/MIME implementations, not custom cryptography.
Provide key/certificate import, fingerprint display, signing, verification,
encryption and decryption. Private keys use the existing protected-secret boundary;
extensions and API keys cannot export them. Never silently fall back to plaintext
when encryption was requested. Distinguish encrypted, valid signature, invalid
signature, and untrusted signer; encryption alone does not prove identity.

Builder security default: API keys and third-party extensions receive encrypted
content and verification metadata, never automatically decrypted plaintext.
Decryption is an owner-controlled reader operation. Automation may request signing
or encryption only within a pending send; the owner approval names the signing
identity and recipient key fingerprints. Approving that send permits that one
cryptographic use, not a general signing/decryption capability. Unavailable or
locked keys leave the request unsent. No standalone automation signing oracle is
included.

Company checkmarks require verified BIMI/VMC evidence and aligned authenticated
sender identity, not a company-name list, avatar, sender-controlled header, or
unverified logo. Only trust authentication results from an explicitly configured
receiving trust boundary. Fetch and validate evidence with bounded requests,
SSRF protection and certificate checks. Show a checkmark only on successful
verification, with a readable explanation. Unknown/unsupported evidence produces
no badge; a signed personal message is not a verified company. Keep this
validation separately testable using controlled DNS/certificate/provider fixtures.

## Errors

Add stable error categories and safe user-facing explanations for authentication,
permission, validation, unavailable provider, timeout, storage, and uncertain
submission. Preserve HTTP error status and structured MCP errors. Map known
protocol status codes and bounded safe server details without exposing raw
responses, credentials, paths, or message bodies. Handle malformed/non-JSON proxy
responses and network disconnects. Recovery actions must not erase unsaved work
or encourage blind retries after an uncertain send.

## Performance and validation

Record baseline before implementation and final integrated just perf-check,
keeping timestamped performance-results history and updating the committed
docs/performance-baseline.json from the final run. Add CI artifact upload only
if CI is configured. Compare matching workload/build/environment metadata.

Extend measurement to the actual Linux Tauri/WebKit process tree, with separate
idle, startup, read, compose, refresh, retained/peak RSS and CPU-time observations.
Chromium results remain useful UI regressions but are not native Tauri evidence.
Research connection reuse, fetch batching, bounded parsing/cache allocations,
background polling, DOM/render work, runtime threads, and release build settings.
Measure candidates separately; keep simple improvements with repeatable benefit,
revert complexity with inconclusive benefit, and document rejected candidates.
Binary size is distinct from runtime memory. Do not claim unmeasured Windows,
Android, real-provider, or physical-device gains.

Run just verify for Rust changes and just perf-check after integrated feature
changes; exercise existing UI/native checks where supported. Add focused tests
for key isolation/revocation/expiry, authorization bypass attempts, approval
replay/expiry/crash uncertainty, cache migration/failure preservation, hostile
HTML, recipient/Bcc/attachment correctness, crypto interoperability, forged
verification evidence, and redacted errors. Test MCP with a real SDK client.

## Builder decisions and sources

Builder chooses compatible dependency versions, module layout, exact UI layout,
and conservative attachment/key/cache quotas, documenting meaningful limits.
Deliver vertical slices through the same contracts rather than placeholder UI.
No system-wide daemon, multi-user hosting, cloud mail sync, POP3, OAuth server,
extension ABI redesign, or unmeasured optimization is included.

- Turso embedded engine: https://github.com/tursodatabase/turso
- MCP elicitation: https://ts.sdk.modelcontextprotocol.io/v2/servers/elicitation
- Tauri build optimization: https://v2.tauri.app/concept/size/
- OpenPGP: https://www.rfc-editor.org/rfc/rfc9580.html
- S/MIME: https://www.rfc-editor.org/rfc/rfc8551.html
- BIMI/VMC: https://bimigroup.org/verified-mark-certificates-vmc-and-bimi/
