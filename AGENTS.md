# Agent instructions

- Use `caveman` with `megacave` mode for intermediate chat updates only. Final replies, including task summaries and answers to user questions, must use plain, natural language without caveman or megacave. Preserve code, commands, paths, API names, error strings, numbers, units, and negations exactly. Honor skill exceptions for clarity/persisted artifacts and explicit user mode changes.
- Add code comments only when needed; write them in plain, natural language without caveman or megacave.
- Always seek helpful installed skills before work. Read relevant `SKILL.md`; follow workflows. Use installed `caveman` skills when applicable; preserve technical facts and required checks.
- Coding: use `ponytail`. Make smallest complete change; reuse patterns. Avoid speculative abstractions, unused dependencies, junk code.
- UI: use Emil Kowalski's installed `emil-design-eng` plus relevant animation/mobile skills. Other tasks: choose matching domain skills.
- Read `docs/README.md` for product/architecture. Use justfiles for repeatable operations. Rust workspace changes: run `just verify`.
- Tests: target changed behavior, important edge cases, regression risks. Avoid redundant/implementation-mirroring tests and excessive suites for trivial changes. Run relevant existing checks.
- Decide routine reversible details autonomously; think through consequences. Ask direction only when missing intent materially changes work. Do not over-ask; maintainer may not know better.
- Before scheduling heavy builds, linkers, tests, or benchmarks, always check available memory and effective limits (including ancestor cgroups), free storage, CPU quota/affinity, and current load/I/O pressure. Do not rely on host RAM totals. Choose concurrency that fits the available resources. On constrained machines, use `CARGO_BUILD_JOBS=1`; do not overlap heavy Cargo builds sharing a target directory. Wait for resource pressure to settle before collecting performance measurements.

- Resource regressions: after feature changes, run `just perf-check` (verification plus three isolated resource samples). Preserve timestamped `performance-results/` history locally and upload results as CI artifacts when CI is available. Refresh `docs/performance-baseline.json` from the final integrated run so git retains a reviewable feature baseline. Compare only matching workload/build/environment metadata; performance deltas are report-only until stable budgets are established. See `docs/performance.md`.
