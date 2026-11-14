# API version N

<!--
Template for docs/extensions/api-versions/API-<N>.md. Read README.md in this folder
first: it says when to add a version and what else to change. This file only shows how
to write the page. Delete every comment like this one in the real page.

Breaking version: name the file API-<N>-breaking.md and add " ⚠️ BREAKING" to the title,
for example "# API version 7 ⚠️ BREAKING". The `breaking` field in api-versions.json
must match.
-->

## Additions

<!--
New things that change nothing existing: capabilities, operations, optional manifest
fields, relaxed limits. An extension can move to this version by changing only its
api_version. One bullet each, naming the exact capability, operation, field or limit.
Write "N/A" if there are none.
-->

- N/A

## Breaking changes

<!--
Only in breaking versions; write "N/A" otherwise. Anything that makes it unsafe to
change api_version without changing code: a changed signature, field or response shape,
a removed capability or operation, a tightened limit. For each, say what it was, what it
is now and how an extension migrates.
-->

- N/A

## Notes

<!--
Anything else worth knowing: how older versions are still served, whether this version
was later discontinued (and by which release), links to the contract pages. "N/A" if
nothing.
-->

N/A
