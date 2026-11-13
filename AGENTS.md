# Agent instructions

Colecto is run by AI agents with a human maintainer. These rules apply to every agent.
Start with [docs/README.md](docs/README.md) (index, technologies, where files live) and
[docs/architecture.md](docs/architecture.md).

## Skills

- Before starting, list the skills installed for *you* (your global skills, for
  example `~/.agents/skills/` or your client's equivalent) and read the `SKILL.md` of
  every one that fits the task. Follow their workflows.
- Interface and interaction work (Svelte components, layout, motion, mobile): use the
  design-engineering skills, for example `emil-design-eng`, plus the relevant
  animation, accessibility and mobile skills, and review with `break-ui` when you
  finish a visible change.
- Rust work: use any Rust, API-design, security or performance skills you have.
- Svelte and TypeScript work: use Svelte or front-end skills you have, alongside the
  design skills above.
- Other tasks: pick the matching domain skills rather than forcing a UI workflow.
- Do not use the `caveman`, `ultracave` or `megacave` styles unless the maintainer asks
  in the current session. Write chat updates, comments and docs in plain, natural
  language.

## Interface work goes into extensions

- Write every UI feature as a bundled extension under `ui/src/extensions/<id>/`, unless
  it truly cannot be one (bootstrapping, permission prompts, credential controls and
  recovery stay in the shell). "It is quicker in the shell" is not a reason.
- Extensions never reach into each other: no imports between extension folders, no
  shared mutable stores, no reading another extension's state. When one extension
  should be usable by others, expose a versioned interface on its bus (the
  `extensions.bus` capability, see [docs/extensions/README.md](docs/extensions/README.md)),
  for example the request `v1/add_button` rather than `add_button`.

## How to work

- Make the smallest complete change. Reuse existing patterns. No speculative
  abstractions, unused dependencies, empty frameworks or boilerplate nobody maintains.
  Remove code you supersede.
- Decide routine, reversible details yourself and state your assumptions. Ask only
  when missing intent would materially change the work; the maintainer may not know
  better.
- Never describe a planned feature, platform or security property as working. Report
  what you verified and what you did not.
- Add code comments only where the reason is not obvious, in plain language.

## Documentation

Document as you go. Every relevant detail belongs in the docs, but nothing more.

- Update the page that owns the topic (see the index in `docs/README.md`) in the same
  change as the code. Add a new page only when no existing page fits, and link it from
  the index.
- Write for humans and agents alike: a one-line summary at the top, short sections
  with descriptive headings, tables for limits, options and mappings, and exact names
  of files, routes, fields and commands.
- Record behaviour, limits, security boundaries and the reasons behind non-obvious
  decisions. Do not record session logs, test counts or history; git keeps those.
- Keep [docs/roadmap.md](docs/roadmap.md) current when you finish or start a roadmap
  item.

## Checks

- Use the `just` recipes for repeatable operations (`just` lists them).
- Rust changes: run `just verify`. UI changes: also `just test browser`.
- Tests target changed behaviour, important edge cases and regression risks. Avoid
  redundant, implementation-mirroring or oversized suites for small changes.
- After feature changes, run `just perf-check` (verification plus three isolated
  resource samples). Keep the timestamped history in `docs/performance/results/`
  locally, and refresh `docs/performance/baseline.json` from the final integrated run.
  Compare only runs with matching workload, build and environment metadata; deltas are
  report-only for now. See `docs/performance/README.md`.

## Machine resources

Before heavy builds, linkers, tests or benchmarks, check available memory and the
effective limits (including ancestor cgroups), free storage, CPU quota and affinity,
and current load and I/O pressure; do not rely on host RAM totals. Choose concurrency
that fits. On constrained machines use `CARGO_BUILD_JOBS=1` and never overlap heavy
Cargo builds that share a target directory. Let resource pressure settle before
taking performance measurements.

## Shipping

Work goes into `develop`; `main` only moves when a release ships (see "Branches" in
[docs/development.md](docs/development.md)). When a task is done and its checks pass,
commit, push and merge it yourself: branch off `develop`, open a pull request against
`develop` (`gh pr create --base develop`), and merge it once the required checks pass.
Wait for a human only when something truly needs one: failing checks you cannot fix,
merge conflicts with unclear intent, missing access, or destructive history changes.

Ship a release only when the maintainer asks for one in the current session, for
example "ship a new release". Then follow "Shipping a release" in
[docs/development.md](docs/development.md): read what changed since the last tag,
write the release notes, choose a minor or patch version yourself, bump it, merge
`develop` into `main`, and push the tag. Never pick a major version unless the
maintainer asks for one. Never tag or publish at any other time.

When you merge a change that users would notice, remind the maintainer that a release
might be worth shipping. Say it in plain words, twice: once when you open the pull
request, before waiting on CI, and again in your final message after the merge. CI
takes a while, so the first reminder may be the one they see.
