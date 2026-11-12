# C ABI review

This review covers every export in `src/lib.rs` and `include/shape.h`, the
shared SDK request/worker boundary, and the Node, Python and Go consumers.

## Findings and changes

- **Low — incomplete load guard scope.** The previous `shape_generator_load`
  guard covered `Generator::load`, but slice construction and the final boxed
  handle allocation were outside it. The guard now encloses both. Ordinary
  allocation failure still aborts; this change does not claim to catch OOM,
  invalid-pointer UB, or an aborting panic. No user-triggerable unwinding panic
  was identified in the previously unguarded operations.
- **Low — incomplete boundary documentation.** The module description incorrectly
  said every input was NUL-terminated. The header did not explicitly distinguish
  first-NUL string semantics from full-length artifact semantics, prohibit
  result mutation before `CString::from_raw`, or state that input storage must
  remain stable during calls. These contracts are now explicit, including
  same-library allocation/free pairing and results outliving their generator.
- **Medium — regression coverage gap.** The previous two unit tests exercised
  only errors and stateless version calls. The default C crate tests now load a
  public artifact with two real signal routes and exercise all generator methods,
  successful/error ownership, byte-copy lifetime, independent handles, and
  shared-handle calls followed by joined-thread teardown. The consumer smoke
  check links and runs real C99 and C++11 code against shared and static output.

These are compatible boundary hardening and test changes, not an ABI redesign.
No signatures, exported names, handle layout promises, command names, error
codes, or success envelope shapes have changed. `serde_json` is a test/example
dependency only. No production dependency or shared engine change is needed.

## Export-by-export audit

| Export | Contract and implementation reviewed |
| --- | --- |
| `shape_version` | Compile-time NUL-terminated static `CStr`; no allocation, lazy initialization, or destructor. Stable pointer, never passed to `shape_free`. |
| `shape_call` | C strings are read inside `guarded`; NULL command and invalid UTF-8 return `invalid_request`. NULL/empty/whitespace requests become `{}`; malformed or non-object JSON is rejected by shared SDK validation. Unknown names return `unknown_command`. Result is independently allocated JSON. |
| `shape_generator_load` | Clears writable `error_out` before validation; never releases its previous value. NULL and lengths above `isize::MAX` are rejected before creating a slice. Exactly `len` bytes are consumed, no terminator required. `Generator::load` copies input and validates on a large-stack worker. Error strings use the library allocator; success returns one boxed handle and a NULL error slot. A NULL error slot is supported on success and failure. |
| `shape_generator_call` | NULL handle/method and invalid UTF-8 are rejected under `guarded`. All four methods use shared request validation. The handle borrows immutable validated state in an `Arc`; runtime concurrent decode/schema and compile-time `Send + Sync` checks cover sharing. Results do not borrow the handle. |
| `shape_generator_free` | NULL is ignored; a live uniquely owned pointer is reconstructed with `Box::from_raw`. The current immutable generator, Arc, artifact and collections have no custom panicking drop path. The caller must wait for all calls before freeing. |
| `shape_free` | NULL is ignored; `CString::from_raw` matches `CString::into_raw` in `owned`, including errors. Strings must be unchanged and freed exactly once by this library instance. Static version strings and host-allocated strings are excluded. Current drop paths have no user callbacks or recoverable panics. |

`guarded` contains request borrowing, JSON parsing, engine dispatch, and envelope
construction. Conversion to `CString` follows the guard; its fallible interior-NUL
case has a fixed valid fallback. JSON serialization escapes raw NULs. Allocation
failure aborts rather than unwinds. Engine calls that need substantial stack run
through `run_with_large_stack`, whose worker catches unwinding engine panics.
The load boundary catches caller-thread failures during copy/load as before,
and now also encloses slice construction and final handle allocation.
The free exports have no error channel and no intentional panicking operation
under their documented preconditions.

## Consumer and distribution review

- Node `addon.c`: copies incoming artifact bytes and strings, frees library
  results with `shape_free`, and uses a tagged `Holder` with pending-call
  accounting to postpone generator release until async work is finished.
  Holder accounting happens on the JS thread; workers borrow immutable native
  state. Async completion copies result text before freeing it.
- Python `_native.py` / `_generator.py`: owned pointers are `c_void_p`, so ctypes
  does not discard their addresses; `take` copies and frees in `finally`.
  Input bytes are kept alive across load. A condition variable rejects new
  calls during close and waits for active calls before release.
- Go `generator.go` / `shape.go`: host inputs use C malloc/free while native
  results use `shape_free`. A read/write mutex and `runtime.KeepAlive` keep the
  handle alive across calls; close takes the write lock. `C.size_t` matches
  Rust `usize`; the opaque handle is never inspected by C callers.
- No mismatched signature, allocator pairing, or close/call assumption was
  found in these paths. Host wrappers reject embedded NUL strings, whereas
  direct C callers receive the normal first-NUL semantics documented here.
- `shape.h` uses standard integer/size types and `extern "C"` guards. No Rust
  struct representation crosses the boundary. Both crate outputs are declared
  (`cdylib`, `staticlib`). On macOS the existing SDK build recipe supplies
  `-C strip=none` and `@rpath/libshape.dylib`; the smoke script copies the dylib
  beside the executable and resolves it with `@loader_path`. Linux uses
  `$ORIGIN`. Static linking uses the documented platform system libraries.
  This review does not alter another checkout's installed native outputs.

## Reproduction

Default tests consume the committed fixture directly, without a private capture,
Python, Node, environment variables, or fixture-generation work:

```sh
cargo test --release -p shape-c --lib
```

Regenerate fixtures only when needed:

```sh
cargo test --release -p shape-c --test regenerate_fixture -- --ignored
```

The generator reuses the repository's public synthetic JS source and reference
codecs, following `tests/decode_workflow.rs`; it saves only the artifact and
capture needed by consumers. Its three signals are `DeviceMemory` (number),
`StorageAccessApiPresence` (packed flags) and `ConstantEmptyString` (string).
It does not duplicate those source fixtures.

After a fresh native build, run:

```sh
# macOS, using the existing relocatable-library build recipe:
cargo rustc --release -p shape-c -- \
  -C strip=none -C link-arg=-Wl,-install_name,@rpath/libshape.dylib
# Linux: cargo build --release -p shape-c
sdk/c/tests/run-smoke.sh /absolute/path/to/target/release
```

The smoke script places all mutable output in a temporary directory and tests
both C99 and C++11 with strict compiler warnings, for both link modes.

## Remaining limits

Pointers must identify live, aligned storage of the documented size. String
terminators must be within one allocation. Callers must not race input mutation,
output access/free, or handle free with in-flight calls, and must not reuse freed
pointers. Invalid pointer ranges, double frees and concurrent frees are undefined
behavior, not recoverable JSON errors. Returned storage must be freed before
unloading the library that created it. The API is blocking and not async-signal-safe.

No production command intentionally panics, and no input-triggerable panic was
identified for deterministic injection. The regression test injects an unwinding
panic into the actual private `guarded` helper used by the two call exports,
then frees its panic envelope through `shape_free` and verifies another exported
call succeeds. This tests the guard, not an end-to-end exported fault-injection
path. No production test hook or public fault-injection command is added.
Process aborts, stack overflow, OOM and builds configured with `panic=abort`
remain outside unwinding recovery. Windows and Linux runtime behavior require
checks on those hosts; the smoke script supports macOS and Linux.

## External test and distribution observations

- **Resolved — Node fixture assumption.** The earlier review saw
  `sdk/node/test/generator.test.js` assert a fixed decoded count against the
  public fixture. The suite now checks the live `metadata.decoded` against the
  named signal map instead of a constant, and the public fixture decodes three
  signals (number, packed flags and string). The opt-in Node generator suite is
  expected to run against it and skip only when the fixture variables are
  absent.
- **Low, distribution validation limit — deployment targets.** Both static
  consumers link and run, but the linker reports that reused cached native C
  dependency objects target macOS 27.2 while the consumer link targets 27.0.
  This review certifies execution on this host, not older macOS compatibility.
  A release intended for older deployment targets needs all native dependencies
  rebuilt consistently for that target. No linker warnings were suppressed.

## Executed validation (2026-11-10, Darwin arm64)

Build output was isolated in `/tmp/shape-c-abi-review-target`, initially cloned
from an existing release dependency cache; engine and C crate sources were
freshly rebuilt. Tools: Rust 1.94.0, Node 22.22.2, Go 1.27.4.

| Check | Exact invocation / result |
| --- | --- |
| C ABI unit tests | `CARGO_TARGET_DIR=/tmp/shape-c-abi-review-target cargo test --release -p shape-c --lib`: 6 passed, 0 failed, 0 ignored. Re-run after the deterministic capture and exact f64-bit assertion changed. |
| Fixture generation | Historical example invocation in the C review passed and produced identical SHA-256 hashes on repeat. The utility now lives in `tests/regenerate_fixture.rs`; use the explicit ignored-test invocation above. |
| Fresh native output | `CARGO_TARGET_DIR=/tmp/shape-c-abi-review-target cargo rustc --release -p shape-c -- -C strip=none -C link-arg=-Wl,-install_name,@rpath/libshape.dylib`: passed, shared library and archive produced. |
| Actual C/C++ consumers | `sdk/c/tests/run-smoke.sh /tmp/shape-c-abi-review-target/release`: all 4 C99/C++11 × dynamic/static consumers passed, 0 skips. Re-run after the capture changed. |
| Export/install-name inspection | `nm -gU` shows all six `shape_*` functions; `otool -D` reports `@rpath/libshape.dylib`; `otool -L` shows the platform dependencies. |
| Python | `uv run --no-project python -B -m unittest discover -s sdk/python/tests -v`, with `SHAPE_LIBRARY` set to the fresh dylib and `SHAPE_TEST_ARTIFACT`/`SHAPE_TEST_HEADERS` set to these fixtures: 30 tests, 29 passed, 1 skipped (artifact contains no integer above 2^53), 0 failed. Re-run after the capture changed. |
| Go | From `sdk/go`, `go test -tags shape_external -count=1 -v ./...`, with `CGO_LDFLAGS='-L/tmp/shape-c-abi-review-target/release -lshape -Wl,-rpath,/tmp/shape-c-abi-review-target/release'` and both fixture variables: 16 top-level tests and 5 subtests passed, 0 skips/failures. Re-run after the capture changed. |
| Node default | Built an unchanged temporary copy of `sdk/node` against the fresh dylib/header, then `node --test /tmp/shape-c-abi-review-node/test/index.js` with empty fixture variables: 19 passed, 4 opt-in skips, 0 failed. |
| Node optional fixture run | Same temporary copy with both fixture variables, `node --test --test-name-pattern='^(?!generator decode / explain / schema \(sync\)).*$' /tmp/shape-c-abi-review-node/test/index.js`: the intended exclusion did not take effect. Actual result: 21 passed, 1 failed (`2 !== 75`), 1 skipped (artifact integer precision). This failure is retained above, not counted as a passing check. |
| Node synchronous public fixture probe | A separate temporary script loaded the final two-signal fixture, asserted count 2 and DeviceMemory 32, explained it, compared encoded capture bytes, edited DeviceMemory to 8 and decoded it, read schema, and verified close rejects further calls: passed. |
| Source checks | `git diff --check`, `bash -n sdk/c/tests/run-smoke.sh`, and `rustfmt --check` on the C Rust sources and the then-current fixture generator: passed. |

The C test asserts the exact decoded `numberBits` value `4629700416936869888`
(32.0's f64 representation), so precision above 2^53 is exercised even though
this small artifact itself has no such constant and the existing Python/Node
artifact-object precision tests skip. Existing engine unused/dead-code warnings,
Go duplicate-link-flag warnings and macOS deployment-target linker warnings
were observed. No unrelated giant test suite was run. Linux and Windows were
not executed on this macOS host.

## Additive operation contract v2

The v2 work adds symbols without removing or changing the six original C
exports. `shape_call_v2` routes all 67 operation IDs through a separate provider;
legacy `shape_call`, its 21-command list, and its CLI-oriented defaults remain.
No wrapper changes process environment. The three cache policy forms are
validated before source compilation, and loaded inputs reuse memory separately
from disk cache.

`ShapeArtifact` is a typedef of the existing opaque generator allocation, with
load/call/free aliases and direct decode/encode/explain/schema functions. All
operations share the original immutable validated artifact. Cross-calling old
and new aliases on one live allocation is supported; freeing through either
alias releases it exactly once. The additional `artifact` method serializes the
executable document for explicit caller persistence.

`ShapeKernel` owns one parsed kernel. Its calls borrow the artifact and permit
concurrent readers; the caller must wait for every call before freeing. Program
tracing verifies exact raw-source identity. Kernel load checks NULL and
PTRDIFF_MAX before constructing a slice. Loads and calls retain panic envelopes;
OOM, invalid pointers and use-after-free remain outside that boundary.

The new ABI tests exercise aliases, direct calls, request errors, kernel input
copying, mismatched source identity, results outliving handles, and legacy
compatibility. The C99/C++11 smoke program links all four dynamic/static variants.
Both standalone README programs compile with warnings as errors. The loaded
workflow runs against the public fixture and exercises schema, decode, explain,
unchanged encoding, named editing, and decoding the edited headers. Existing
fixture integers remain exact JSON tokens throughout.
