#!/usr/bin/env bash
#
# Measure the unified `shape deob` command directly, without Cargo or a terminal
# in the loop. This helper measures deobfuscation only; it is not a VM benchmark.
#
# Usage:
#   docs/benchmark.sh <release-shape> <input.raw.js> [warm-runs]
#
# The first invocation is reported separately from the warm runs. With a small
# sample this script reports the median and min..max range only; it deliberately
# does not print a p95 from a handful of runs. stdout is redirected to
# /dev/null so the measurement is parsing/analysis/codegen plus process startup,
# not terminal rendering. Run each seed more than once to characterize I/O
# cache warmth, and do not treat a single reading as a stable baseline.
set -euo pipefail

BIN=${1:?usage: docs/benchmark.sh <release-shape> <input.raw.js> [warm-runs]}
SEED=${2:?usage: docs/benchmark.sh <release-shape> <input.raw.js> [warm-runs]}
RUNS=${3:-6}

[[ -x "$BIN" ]] || { echo "not executable: $BIN" >&2; exit 2; }
[[ -f "$SEED" ]] || { echo "no such seed: $SEED" >&2; exit 2; }

python3 - "$BIN" "$SEED" "$RUNS" <<'PY'
import os, statistics, subprocess, sys, time

binary, seed, warm_runs = sys.argv[1], sys.argv[2], int(sys.argv[3])
devnull = open(os.devnull, "wb")
times = []
for _ in range(warm_runs + 1):
    start = time.perf_counter()
    result = subprocess.run([binary, "deob", seed], stdout=devnull, stderr=subprocess.PIPE)
    times.append((time.perf_counter() - start) * 1000.0)
    if result.returncode != 0:
        sys.exit(f"shape deob failed ({result.returncode}): {result.stderr[:400]!r}")

warm = times[1:]
print(f"binary   {binary}")
print(f"seed     {seed} ({os.path.getsize(seed)} bytes)")
print(f"runs     first + {warm_runs} warm, stdout -> /dev/null")
print(f"first    {times[0]:.1f} ms")
print(f"warm med {statistics.median(warm):.1f} ms")
print(f"warm rng {min(warm):.1f}..{max(warm):.1f} ms")
print("warm all " + ", ".join(f"{millis:.1f}" for millis in warm))
PY
