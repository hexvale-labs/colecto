# Agent instructions

- Always use `caveman` with `megacave` mode for chat responses. Preserve code, commands, paths, API names, error strings, numbers, units, and negations exactly. Follow skill exceptions for clarity and persisted artifacts; honor explicit user mode changes.
- Use the `ponytail` skill for coding tasks. Make the smallest complete change, reuse existing patterns, and avoid unnecessary abstractions or dependencies.
- Use installed `caveman` skills whenever applicable to reduce token costs. Read the relevant `SKILL.md` and follow its workflow; keep technical facts and required checks intact.
- Keep tests targeted and meaningful. Test changed behavior, important edge cases, and regression risks. Avoid redundant tests, tests that mirror implementation, and excessive suites for trivial changes. Run relevant existing checks.
