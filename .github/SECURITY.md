# Security policy

## Reporting a vulnerability

Please report security problems privately through GitHub:
**[Report a vulnerability](https://github.com/hexvale-labs/colecto/security/advisories/new)**
(the **Security** tab → **Report a vulnerability**).

Do not open a public issue, pull request or discussion for a vulnerability. A private
report lets us talk it through with you, fix it and publish an advisory, crediting you
if you like.

Helpful details:

- what is affected (the app, the daemon, the MCP bridge or an extension) and which
  version or commit
- how to reproduce it, with a proof of concept if you have one
- what an attacker could do with it

We will reply in the advisory. Colecto is a small volunteer project, so there is no
bug bounty and no guaranteed response time, but security reports come first.

## Supported versions

Only the latest release, and `main`, receive security fixes.

## Scope

Colecto handles mail credentials and untrusted mail, so we especially want to hear about:

- reading or changing mail, drafts, credentials or settings without the owner token
  or a properly scoped API key
- HTML mail escaping its sandbox, running script, or loading remote content the owner
  did not allow
- third-party extensions escaping the WebAssembly sandbox or exceeding their grants
- credentials or tokens appearing in logs, URLs or responses
- automation sending mail without the owner's approval
