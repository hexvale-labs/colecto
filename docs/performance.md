# Resource measurements

Run `just perf-check` for feature changes. It runs routine verification, builds the
local host/UI, tests the recorder, and records three isolated resource runs.
`just perf` repeats only the builds, recorder tests, and measurements. Install the
browser once with `just setup-browser`. Python 3, Node, and Linux `/proc` are
required; no additional Python packages are needed.

The workload starts each Host with a fresh temporary data directory and strips
all inherited `COLECTO_*` configuration. It cannot connect your real mail account
or modify existing drafts. Chromium opens sample mail, then reads and saves 12
small drafts through the real UI. Measurements begin at process startup and end
after a one-second settle following the workload. An initial one-second idle
sample follows browser startup. Compilation is outside the measured interval.
Temporary processes and data are cleaned up even when a workload fails.

Each timestamped JSON under `performance-results/` retains the three raw runs,
mean/median, commit/dirty status, executable/UI hashes, machine/kernel/CPU, workload,
build profile, and browser-driver/Node versions. These files are ignored by git
but persist locally. Keep or archive this directory for long-term trends; in CI,
upload `performance-results/*.json` as retained artifacts and restore prior
history before measuring. CI runs `just verify` and `just ui-test`, but not `just perf`, so it records no history yet.
The committed initial sample in `docs/performance-baseline.json` is evidence from
one development machine, not a universal budget. Refresh that committed sample
from the final integrated run after feature changes; git history then retains
a reviewable sequence even when local raw history is unavailable.

The daemon and Node/Chromium process tree are measured separately:

- Daemon CPU time is Linux user plus system CPU time, not a fluctuating CPU
  percentage. One-second idle CPU is reported separately. Peak RSS comes from the process high-water mark, while idle and
  retained RSS are actual live snapshots. Growth is retained minus initial RSS.
- Browser CPU is the sum of the highest observed CPU time for each process in
  the Node/Chromium tree. Idle/retained RSS sums live process RSS. Peak browser
  RSS is the highest sampled aggregate, not the sum of process high-water marks.
  RSS includes shared pages in each process, so this is not unique physical RAM.
- The 50 ms sampler may miss short-lived browser subprocesses or brief browser
  peaks. Settling for one second does not force garbage collection. Saving drafts
  intentionally retains bounded data; these measurements alone do not prove a
  leak or measure native WebView/Android behavior.

The recorder compares the latest matching machine, profile, workload, cycle
count, UI mode, and browser runtime, reporting median changes without failing on
performance thresholds. Three runs give a quick signal, not statistical proof.
Use a quiet machine, inspect raw samples, and repeat suspicious changes. Set
budgets after several stable runs; do not gate changes on a guessed RAM limit.

For lower-noise daemon-only or release comparisons:

```sh
python3 scripts/resource_bench.py --skip-ui
cargo build --release -p colecto-host --locked
python3 scripts/resource_bench.py --binary target/release/colecto-host
```

The daemon-only workload fetches sample inbox mail 12 times; it is a distinct
comparison group. Pass `--runs`, `--cycles`, or `--history /persistent/path` when
needed. Compare builds with the same settings. Release and debug results are
kept separate. Build an alternate binary before passing it with `--binary`.

Keep at least one real provider and physical Android device in later performance
validation: sample mail does not represent IMAP reconnects, MIME-heavy mail,
large folders, device battery drain, or FCM/UnifiedPush delivery.

### Embedded mail cache

Mail snapshots use the official local Rust `turso` 0.7.2 engine with default
features disabled, avoiding its optional process-wide mimalloc allocator. Local
operations use a narrow `futures-executor` adapter: no nested Tokio runtime and no
per-account runtime/thread. Cached folder lists and retained reads use independent
account-bound snapshots, so provider network work does not hold their lock.
Inbox and Sent share the existing 100-message/12 MB serialized-content budget per
account; when the newly fetched folder needs the space, its transaction
evicts the other folder. Search results stay in the existing bounded reader cache.
Bounded serialization checks aggregate message bytes before allocating encoded
content and stops encoding at the byte limit. The database and WAL are created
only after the containing directory is private.

Run `just perf-check` for the final integrated security feature comparison and
refresh the baseline from that run, preserving earlier timestamped samples.

The existing plaintext cache is not encrypted by the credential vault. Explicit
Disconnect and vault Lock clear the active memory snapshots; failure to reach a
provider leaves them intact. Retained database snapshots are selected again only
after the configured provider identity reconnects.
