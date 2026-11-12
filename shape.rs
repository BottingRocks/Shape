//! Unified `shape` command-line entry point.
//!
//! `main` and top-level dispatch only. Every command lives in the binary-only
//! `cli/` folder; the library (`src/`) holds no command-line code. The command
//! line orchestrates shared library analysis; it never shells out to another
//! executable and never executes kernel JavaScript or WASM.

/// Tracing allocates and frees many small values (nodes, snapshots,
/// maps): mimalloc serves those faster than the system allocator.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod cli;

use std::{env, process::ExitCode};

fn main() -> ExitCode {
    shape::run_with_large_stack(run)
}

fn run() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("shape: missing command\n{}", cli::USAGE);
        return ExitCode::from(2);
    };
    match command.as_str() {
        "-h" | "--help" => cli::print_help(cli::USAGE),
        "--version" => cli::print_help(&format!("shape {}", env!("CARGO_PKG_VERSION"))),
        "deob" => cli::deob::run(args.collect()),
        "report" => cli::report::run(args.collect()),
        "kernel" => cli::kernel::run(args.collect()),
        "ops" => cli::ops::run(args.collect()),
        "show" => cli::show::run(args.collect()),
        "signals" => cli::signals::run(args),
        "naming" => cli::naming::run(args),
        "seeds" => cli::seeds::run(args),
        "pin" => cli::payload::run_pin(args),
        "headers" => cli::payload::run_headers(args),
        "payload" => cli::payload::run_payload(args),
        "encode" => cli::payload::run_encode(args),
        "decode" => cli::decode::run(args.collect()),
        "pack" => cli::payload::run_pack(args),
        "unpack" => cli::payload::run_unpack(args),
        "gen" => cli::payload::run(args),
        "trace" => cli::trace::run(args),
        other => {
            eprintln!("shape: unknown command {other}\n{}", cli::USAGE);
            ExitCode::from(2)
        }
    }
}
