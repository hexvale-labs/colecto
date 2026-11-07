# Agent instructions

- Always use `caveman` with `megacave` mode for chat. Preserve code, commands, paths, API names, error strings, numbers, units, and negations exactly. Honor skill exceptions for clarity/persisted artifacts and explicit user mode changes.
- Always seek helpful installed skills before work. Read relevant `SKILL.md`; follow workflows. Use installed `caveman` skills when applicable; preserve technical facts and required checks.
- Coding: use `ponytail`. Make smallest complete change; reuse patterns. Avoid speculative abstractions, unused dependencies, junk code.
- UI: use Emil Kowalski's installed `emil-design-eng` plus relevant animation/mobile skills. Other tasks: choose matching domain skills.
- Read `docs/README.md` for product/architecture. Use justfiles for repeatable operations. Rust workspace changes: run `just verify`.
- Tests: target changed behavior, important edge cases, regression risks. Avoid redundant/implementation-mirroring tests and excessive suites for trivial changes. Run relevant existing checks.
- Decide routine reversible details autonomously; think through consequences. Ask direction only when missing intent materially changes work. Do not over-ask; maintainer may not know better.
