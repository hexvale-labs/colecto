//! Process-wide logging for the desktop app, the daemon and the MCP bridge.
//!
//! Logs go to stderr. `COLECTO_LOG` (or `RUST_LOG`) takes an `EnvFilter` directive such as
//! `debug` or `colecto_core=trace`. Under systemd each line carries a `<N>` syslog priority
//! prefix so journald records real levels; elsewhere (Docker, terminals) lines are timestamped.
use std::{io::IsTerminal, path::Path};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, FmtContext, FormatEvent, FormatFields, format::Writer},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
};

const DEFAULT_FILTER: &str = "warn,colecto_host=info,colecto_core=info,colecto_app=info,colecto_daemon=info,colecto_mcp=info";

/// Log to stderr only.
pub fn init() {
    init_with_files(None);
}

/// Log to stderr and, when `directory` is given, to daily `colecto.*.log` files kept for a week.
/// Later calls are ignored.
pub fn init_with_files(directory: Option<&Path>) {
    let filter = EnvFilter::try_from_env("COLECTO_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    // systemd sets JOURNAL_STREAM when stdout/stderr are connected to the journal.
    let stderr = if std::env::var_os("JOURNAL_STREAM").is_some() {
        fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .event_format(Journald)
            .boxed()
    } else {
        fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(std::io::stderr().is_terminal())
            .boxed()
    };
    let files = directory.and_then(|directory| {
        tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("colecto")
            .filename_suffix("log")
            .max_log_files(7)
            .build(directory)
            .ok()
            .map(|files| fmt::layer().with_writer(files).with_ansi(false))
    });
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(files)
        .try_init();
}

/// journald adds its own timestamp and parses a leading `<priority>` (sd-daemon(3)).
struct Journald;

impl<S, N> FormatEvent<S, N> for Journald
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        write!(writer, "<{}>", priority(*event.metadata().level()))?;
        fmt::format()
            .without_time()
            .with_ansi(false)
            .format_event(context, writer, event)
    }
}

fn priority(level: Level) -> u8 {
    match level {
        Level::ERROR => 3,
        Level::WARN => 4,
        Level::INFO => 6,
        _ => 7,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn journald_priorities_follow_syslog_levels() {
        use tracing::Level;
        let priorities = [
            Level::ERROR,
            Level::WARN,
            Level::INFO,
            Level::DEBUG,
            Level::TRACE,
        ]
        .map(super::priority);
        assert_eq!(priorities, [3, 4, 6, 7, 7]);
    }
}
