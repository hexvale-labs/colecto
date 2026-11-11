# Agent instructions

- Temporarily disabled: do not use the `caveman` skill or `megacave` mode, even if a skill says to use it for coding tasks. Write all chat updates and replies in plain, natural language. Only use caveman if the user explicitly asks for it in the current session.
- Add code comments only when needed; write them in plain, natural language.
- Always seek helpful installed skills before work. Read relevant `SKILL.md`; follow workflows.
- Coding: use `ponytail`. Make smallest complete change; reuse patterns. Avoid speculative abstractions, unused dependencies, junk code.
- UI: use Emil Kowalski's installed `emil-design-eng` plus relevant animation/mobile skills. Other tasks: choose matching domain skills.
- Read `docs/README.md` for product/architecture. Use justfiles for repeatable operations. Rust workspace changes: run `just verify`.
- Tests: target changed behavior, important edge cases, regression risks. Avoid redundant/implementation-mirroring tests and excessive suites for trivial changes. Run relevant existing checks.
- When a task is done and its checks pass, commit, push, and merge it automatically: branch off `main` if needed, push, open a PR, and merge it once required checks pass. Do not wait for approval unless something truly needs a human (failing checks you cannot fix, merge conflicts with unclear intent, missing access, or destructive history changes).
- Decide routine reversible details autonomously; think through consequences. Ask direction only when missing intent materially changes work. Do not over-ask; maintainer may not know better.
- Before scheduling heavy builds, linkers, tests, or benchmarks, always check available memory and effective limits (including ancestor cgroups), free storage, CPU quota/affinity, and current load/I/O pressure. Do not rely on host RAM totals. Choose concurrency that fits the available resources. On constrained machines, use `CARGO_BUILD_JOBS=1`; do not overlap heavy Cargo builds sharing a target directory. Wait for resource pressure to settle before collecting performance measurements.

- Resource regressions: after feature changes, run `just perf-check` (verification plus three isolated resource samples). Preserve timestamped `docs/performance/results/` history locally and upload results as CI artifacts when CI is available. Refresh `docs/performance/baseline.json` from the final integrated run so git retains a reviewable feature baseline. Compare only matching workload/build/environment metadata; performance deltas are report-only until stable budgets are established. See `docs/performance/README.md`.
