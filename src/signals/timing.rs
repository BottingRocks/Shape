//! Ad-hoc timing spans for profiling (`SHAPE_SIGNAL_TIMES=1` prints each
//! span's wall time to stderr when it ends). Off by default.

use std::sync::OnceLock;
use std::time::Instant;

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SHAPE_SIGNAL_TIMES").is_some())
}

pub(crate) struct Span {
    name: &'static str,
    start: Option<Instant>,
}

pub(crate) fn span(name: &'static str) -> Span {
    Span {
        name,
        start: enabled().then(Instant::now),
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            eprintln!("TIME {:>9.1} ms  {}", start.elapsed().as_secs_f64() * 1e3, self.name);
        }
    }
}
