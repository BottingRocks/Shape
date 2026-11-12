//! Opt-in, stderr-only profiling for the deobfuscation pipeline.
//!
//! Enable with `SHAPE_DEOB_PROFILE=1` (or any value other than `0`). When
//! disabled — the default — every instrumented step pays only one cached
//! environment lookup and the CLI stays byte-for-byte silent. When enabled,
//! `report()` prints one sorted table of accumulated label timings to stderr;
//! stdout, exit status, and the generated program are untouched.
//!
//! Timings come from `Instant` pairs recorded around real pipeline steps (and
//! around every Oxc parse/analyze/codegen call), so `count` doubles as the
//! call count for those shared operations.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Entry {
    label: &'static str,
    total: Duration,
    count: u64,
}

static ENABLED: OnceLock<bool> = OnceLock::new();
static ENTRIES: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
static REPORTED: OnceLock<()> = OnceLock::new();

/// Whether profiling output is requested for this process.
pub fn enabled() -> bool {
    *ENABLED
        .get_or_init(|| std::env::var_os("SHAPE_DEOB_PROFILE").is_some_and(|value| value != "0"))
}

fn entries() -> &'static Mutex<Vec<Entry>> {
    ENTRIES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Add one observation for `label`. Cheap no-op when profiling is disabled.
pub fn record(label: &'static str, elapsed: Duration) {
    if !enabled() {
        return;
    }
    let mut guard = entries().lock().expect("profile entries poisoned");
    if let Some(entry) = guard.iter_mut().find(|entry| entry.label == label) {
        entry.total += elapsed;
        entry.count += 1;
    } else {
        guard.push(Entry {
            label,
            total: elapsed,
            count: 1,
        });
    }
}

/// Start a timing span, or `None` when profiling is disabled.
pub fn start() -> Option<Instant> {
    enabled().then(Instant::now)
}

/// Record one zero-duration observation, for call/iteration counts where a
/// timer would add noise. Cheap no-op when profiling is disabled.
pub fn tally(label: &'static str) {
    record(label, Duration::ZERO);
}

/// Finish a span opened by [`start`]. No-op for `None`.
pub fn done(label: &'static str, start: Option<Instant>) {
    if let Some(start) = start {
        record(label, start.elapsed());
    }
}

/// Print the accumulated table to stderr. Prints at most once per process.
pub fn report() {
    if !enabled() {
        return;
    }
    if REPORTED.set(()).is_err() {
        return;
    }
    let guard = entries().lock().expect("profile entries poisoned");
    let mut rows: Vec<&Entry> = guard.iter().collect();
    rows.sort_by(|a, b| b.total.cmp(&a.total).then(a.label.cmp(b.label)));
    eprintln!("shape profile (SHAPE_DEOB_PROFILE=1):");
    eprintln!(
        "{:<46} {:>10} {:>7} {:>12}",
        "label", "total_ms", "count", "per_call_us"
    );
    for entry in rows {
        let total_ms = entry.total.as_secs_f64() * 1000.0;
        let per_call_us = total_ms * 1000.0 / entry.count.max(1) as f64;
        eprintln!(
            "{:<46} {:>10.1} {:>7} {:>12.2}",
            entry.label, total_ms, entry.count, per_call_us
        );
    }
}
