# Agent instructions

Colecto is built by AI agents with one human maintainer. These rules apply to every
agent. `CLAUDE.md` is a symlink to this file.

## Before you start

- Read [docs/README.md](docs/README.md) (index, technologies, where files live) and
  [docs/architecture.md](docs/architecture.md).
- List the skills available to you and read the `SKILL.md` of each one that fits the
  task, then follow it. Shared project skills live in `.agents/skills/` (pinned in
  `skills-lock.json`; `.claude/skills` links to the same folder). Your global skills
  apply too.
- For interface work (Svelte components, layout, motion, mobile), use the design skills
  such as `emil-design-eng`, the matching animation and mobile skills, and finish each
  visible change with `break-ui`.

## How to work

- Make the smallest complete change. Reuse existing patterns. No speculative
  abstractions, unused dependencies or boilerplate. Remove code you replace.
- Decide routine, reversible details yourself and state your assumptions. Ask only when
  missing intent would materially change the work.
- Never describe a planned feature, platform or security property as working. Report
  what you verified and what you did not.
- Write chat updates, comments and docs in plain, natural language. Comment code only
  where the reason is not obvious.

## Code rules

### Interface work goes into extensions

- Build every UI feature as a bundled extension in `ui/src/extensions/<id>/`.
  Only bootstrapping, permission prompts, credential controls and recovery stay in the
  shell. "Quicker in the shell" is not a reason.
- Extensions never reach into each other: no imports between extension folders, no
  shared mutable stores, no reading another extension's state. To make an extension
  usable by others, expose a versioned request on its bus (the `extensions.bus`
  capability, see [docs/extensions/README.md](docs/extensions/README.md)), such as
  `v1/add_button`. Bundled bus names use whole-number versions and are listed in
  [docs/extensions/bundled-bus.md](docs/extensions/bundled-bus.md).

### Settings

Put each setting in the Settings section that matches what it changes, not in the
extension that owns it, using `section` (see
[placing settings](docs/extensions/README.md#placing-settings-in-a-core-section)).

| Section | Holds |
| --- | --- |
| **Appearance** | How Colecto looks: text, colour, density, layout of lists and panes |
| **Notifications** | What to be notified about, and how |

### Extension API versions

- Every change to what extensions can rely on adds the next API version in
  `crates/colecto-extension-api/api-versions.json`. Follow
  [docs/extensions/api-versions/README.md](docs/extensions/api-versions/README.md).
- Raise `api_version` only in the extensions that use the new feature.
- Change an existing signature in a new version instead of adding a variant such as
  `request2`. Ask the maintainer before adding a parallel variant.

## Documentation

- Update the page that owns the topic (see the index in `docs/README.md`) in the same
  change as the code. Add a new page only when none fits, and link it from the index.
- Start each page with a one-line summary, then short sections with descriptive
  headings. Use tables for limits, options and mappings, and give exact names of files,
  routes, fields and commands.
- Record behaviour, limits, security boundaries and the reasons for non-obvious
  decisions. Leave out session logs, test counts and history; git keeps those.
- Keep [docs/roadmap.md](docs/roadmap.md) current when you start or finish an item.

## Checks

Use the `just` recipes (`just` lists them; details in
[docs/development.md](docs/development.md#check-changes)).

| Change | Run |
| --- | --- |
| Rust | `just verify` |
| UI | `just verify` and `just test browser` |
| Features | `just perf-check` as well, following [docs/performance/README.md](docs/performance/README.md) |

- Test changed behaviour, important edge cases and regression risks. Skip redundant
  or implementation-mirroring tests.
- Before heavy builds, tests or benchmarks, check free memory against the effective
  cgroup limits (not host RAM), disk space, CPU quota and current load. On constrained
  machines use `CARGO_BUILD_JOBS=1` and never run two Cargo builds on the same target
  directory at once. Let load settle before measuring performance.

## Commits, pull requests and issues

- Write commit messages and pull request titles as
  [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`,
  with `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci` or `chore`,
  and `!` after the type for a breaking change. The scope is optional.
- End every pull request and issue description with one final line naming the models
  that helped with it, for example `Assisted by Opus 5.5 and Gemini 4 Neon.`
- Branch off `develop` and open pull requests against it; `main` moves only when a
  release ships (see [Branches](docs/development.md#branches)). When the checks pass,
  merge the pull request yourself. Wait for the maintainer only for failing checks
  you cannot fix, merge conflicts with unclear intent, missing access or destructive
  history changes.
- When a merged change is something users would notice, tell the maintainer in plain
  words that a release may be worth shipping: once when you open the pull request,
  and again after the merge.

## Releases

Ship a release only when the maintainer asks for one in the current session. Then
follow [Shipping a release](docs/development.md#shipping-a-release), which covers
picking the version, the notes and tagging. Never choose a major version unless the
maintainer asks for one, and never tag or publish at any other time.
