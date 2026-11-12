# Capture acquisition (`shape seeds`)

`shape seeds` acquires Shape-style asynchronous JavaScript launchers and the
generated kernel each launcher loads. It is part of the one root `shape`
executable and the default build. There is no separate acquisition crate,
lockfile, binary, or feature switch.

The implementation lives in [`src/seeds/`](../../src/seeds/). Captured **data** is
different: the default output directory is the repository-root `seeds/`, which
root `.gitignore` ignores. Do not move captured launchers or kernels under
`src/`; only the Rust acquisition source belongs there. Small synthetic files
used by tests remain tracked under [`tests/fixtures/`](../../tests/fixtures/).

## Commands

Build the release executable once as described in
[Build once](../../README.md#build-once); these commands use `shape` directly.

```text
shape seeds fetch  <launcher-url>... [options]
shape seeds fetch  --site <name> [options]
shape seeds parse  <saved.async.js> [--base-url <launcher-url>]
shape seeds parse  <saved.raw.js>
shape seeds import <saved.raw.js> --seed <token> [--out <dir>]
shape seeds import <saved.raw.js> --launcher <saved.async.js> [--out <dir>]
shape seeds sites
shape seeds help [<command>]
```

`shape seeds --help` and `shape seeds <command> --help` describe the exact
option matrix. Commands return 0 on success, 1 for an operational failure, and
2 for a usage error. Help and version requests return 0.

## What a capture produces

One fetch writes a launcher and the kernel it loads, named after the capture
token:

```text
seeds/
  <token>.async.js   launcher: kernel URL, detail.init(...), boot code
  <token>.raw.js     kernel: the VM source analysed by shape
```

An offline parse reports the identity the launcher carries:

```text
saved.async.js: async launcher
  seed        <capture-token>
  kernel url  https://assets.example.test/kernel.js?seed=<capture-token>
```

The kernel is ordinary minified JavaScript. `shape deob` and
`shape trace program` read it; neither executes it. A direct-kernel entry point
produces only `<token>.raw.js` and needs `--seed` on import.

The launcher is also the initializer. Its `detail.init(...)` call supplies the
session, and the parser accepts the saved launcher as a whole:

```sh
shape pin --input reports/program.js --init seeds/<token>.async.js --out reports/seed.json
```

The `session` member of that pinned layout is the parsed session
(`transactionToken`, `alphabet`, `shuffleKey`, eight `keys`, optional `nonce`).
SDK callers can pass the same launcher to `session.from_initializer`. No separate
init extraction is needed, and a session must match the kernel and headers it
came with.

The other two inputs are not produced by `shape seeds`:

- **Headers** are the captured browser response headers, reduced to suffix keys
  and a `siteId`: `{"siteId":"…","headers":{"a":"…","a0":"…",…}}`. See
  [Inputs](../../README.md#inputs) for the shape and the `x-` prefix rule.
- **The session file** is the parsed session above; save it as
  `session.json` or pass the launcher to the SDK.

From there the pipeline is [deobfuscate and inspect the
kernel](../../README.md#deobfuscate-and-inspect-a-kernel), [trace the
program](../../README.md#trace-a-whole-program), then [compile and reuse a
decode artifact](../../README.md#recover-code-and-save-a-decode-artifact).

## Registry and acquisition protocols

`shape seeds sites` prints the current registry without making a request.
[`sites.rs`](../../src/seeds/sites.rs) is the source of truth for entry names,
URLs, protocols, capture query parameters, and request headers. Choose an
entry from that output instead of treating a provider name as a VM type.

`validated` means the entry point has been fetched end to end with this tool.
It does not promise that the remote server will return the same build or
accept a future request. Reference entries remain visible in `sites`, but
`fetch --site NAME` declines them. A registered single-file protocol can also
remain unsupported; inspect the reported protocol before selecting an entry.

| Protocol | What `fetch` expects | Stored output |
| --- | --- | --- |
| Async launcher | A launcher statically loads one generated kernel | `<capture-token>.async.js` and `<capture-token>.raw.js` |
| Direct kernel | Kernel script bytes or supported inline HTML | `<sha256-of-kernel>.raw.js`, without a launcher sibling |

Registry details can change acquisition without changing the interpreter's
semantics:

- A launcher URL may already include `?async`, or be loaded by an earlier
  `?single` script. Use the registered entry rather than reconstructing it.
- The capture query key can differ from `seed`. `parse --site NAME` supplies
  the registered key; `import --launcher` can use `--seed-param KEY`.
- A direct entry may require navigation-style or script-request headers.
  User `-H` values override the registry profile.
- An inline kernel may follow one same-origin challenge refresh. A changing
  final configuration IIFE can be trimmed before computing kernel identity.

For one capture, inspect `shape seeds sites`, then substitute a validated name
for `NAME` in `shape seeds fetch --site NAME`. `--count N` repeats each selected
entry point. The default output is the Git-ignored `seeds/` directory;
`--out DIR` changes it.

For an offline check, parse a launcher you have already saved. Replace the
path and base URL with the saved file and its original URL:

```sh
shape seeds parse /path/to/saved.async.js \
  --base-url 'https://example.test/assets/launcher.js?async'
```

For a recognized launcher, the command reports its capture token and resolved
kernel URL. It writes no files and performs no request. The base URL matters
when the launcher uses relative script URLs. `fetch` performs network requests;
`parse` and `import` work from saved bytes.

`fetch` resolves a launcher's statically proven script load with URL semantics.
It supports both the `document.createElement("script")` / `src` APIs and a
tightly bounded `document.write` form whose reconstructed markup contains one
quoted script element and, at most, the current script's nonce. When a
launcher has static helper loads, exactly one candidate with the configured
query parameter (default `seed`) may win; multiple distinct candidates carrying a capture token
or any unresolved/dynamic candidate remain errors. It then fetches the kernel
and writes only these sibling files. A response consisting of a substantial raw
kernel followed by one structurally separated final IIFE that loads exactly one
helper without a capture token is still classified as a raw kernel; the complete response bytes,
including that helper, are retained and hashed. Token-bearing, dynamic, or competing
helper loads are rejected. The bounded nonce path assumes the browser's
`currentScript.nonce` is a trusted CSP nonce token and only permits it as the
complete value of a `nonce` attribute; arbitrary dynamic markup is rejected.

A **direct-kernel** site serves the kernel at its entry-point URL instead of
behind a launcher. Such a response may end with one per-request top-level IIFE
(the configuration dispatcher) whose token and timestamps change on every
request. That final IIFE is dropped only when the bytes before it independently
classify as a raw kernel. The token is the SHA-256 of the remaining kernel, so
one server build yields one stored capture and a new build is stored once as a
variant. Direct kernels write only `<capture-token>.raw.js`; `import` takes its identity
from `--seed`.

A direct-kernel entry point may also be an HTML document with the kernel inline.
The largest inline `<script>` body that classifies as a raw kernel after the
configuration trim is extracted, and `<script src=...>` bodies are ignored. If
the document is one same-origin `<meta http-equiv="refresh">` interstitial (an
Akamai bot check, for example), that hop is followed once with the first
response's cookies and a `Referer`; a cross-origin refresh is never followed.
This is still `Protocol::DirectKernel`: the entry point is the kernel location,
whether it is served as script bytes or embedded in a document.

```text
seeds/<capture-token>.async.js
seeds/<capture-token>.raw.js
```

`--out DIR` changes the directory and defaults to `seeds`. `--count N`
requests each launcher repeatedly to collect distinct captures; `--delay-ms`
paces those requests. Repeatable positional URLs and repeatable `--site NAME`
entries may be combined. The raw kernel is never overwritten. Repeating the same
capture and bytes reports a duplicate. Reusing a capture token for different kernel
bytes writes a deterministic SHA-256 suffix and applies the same suffix to its
launcher.

The parser is local and uses Oxc 0.120 semantic bindings; it does not execute
JavaScript and does not use Node, npm, or Babel. A relative `script.src` is
resolved against the containing browser document's base URL, not inherently
against the launcher's own URL. A standalone fetch cannot discover that HTML
document or its `<base>` element, so `--base-url` (or `--site`) should be
provided when browser document-base semantics matter; the direct-fetch fallback
is the launcher's final response URL. `parse` reports the resolved kernel URL
and capture token, while `import` validates a saved raw kernel before storing it.
`import` accepts exactly one of `--launcher` (which supplies the capture token) or
`--seed`; supplying neither or both is a usage error.

Request headers can be supplied repeatedly with `-H 'name: value'`, or with the
`--user-agent` and `--referer` shorthands. With no override, both requests use
the user-agent value pinned in `src/seeds/collect.rs`:

```text
Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36
```

The server chooses the variant. A browser-shaped profile has produced a
WebAssembly VM and the older tool identity has produced a JavaScript VM in
observed captures; neither response is guaranteed on a later request. To request
the older tool identity explicitly, override the user agent:

```sh
shape seeds fetch --site NAME --user-agent 'shape/0.1.0'
```

The server chooses the response based on the profile. Timeouts, redirect
limits, and response-size limits are bounded. Credential headers are not
forwarded to a different origin, including a launcher that redirects before
the kernel is selected. Redirects and non-success statuses are validated as
errors. An HTML launcher is rejected; an HTML direct-kernel entry point is
searched for an inline kernel, and one same-origin meta-refresh interstitial is
followed first. Anything that still does not classify is rejected rather than
stored.

Registry headers are applied before user `-H` overrides. They are request
profiles for those entry points, not credentials
or proof that a later server response will classify as a kernel.

To acquire one capture, let the CLI use its ignored default `seeds/`
directory. After replacing `NAME` with a validated registry entry, this
command makes network requests to that entry point:

```sh
./target/release/shape seeds fetch --site NAME --count 1
```

Run `./target/release/shape seeds sites` first to see the names accepted by the
binary you built. The selected protocol determines whether a launcher is
stored alongside the kernel. `--out DIR` selects a different storage location.

An entry point can periodically re-obfuscate its kernel. Fetches across builds
or backend shards can produce distinct bytes and therefore distinct captures.
That is exact-byte acquisition working as specified, not a failed dedupe; it
also means a periodic collector may accumulate variants over time.

Browser automation is outside this tool's scope. The tests do not fetch live
sites; the only network fetch test uses a loopback mock server.

## Import a saved kernel

`shape seeds import` takes capture files you already have and stores them in the
same launcher/kernel layout as `fetch`. It validates that the kernel parses,
takes the seed identity from the launcher's embedded kernel URL (or from
`--seed`), and writes the exact bytes it was given. No `seeds fetch` call is
involved, and the captured JavaScript or WebAssembly is never executed.

A common manual case is a developer who saves a launcher and its VM kernel from
the browser's Network panel, or receives both files from a teammate. Assume the
launcher's embedded kernel URL carries the seed token `abc123`, and the
downloaded files are named after it:

```text
downloads/
  abc123.async.js
  abc123.raw.js
```

Import the pair, with the launcher supplying the identity:

```sh
shape seeds import downloads/abc123.raw.js \
  --launcher downloads/abc123.async.js \
  --out seeds/imported
```

`import` reads the seed token from the **launcher**, not from the `abc123` file
names. It does not fetch the kernel URL and does not cryptographically
authenticate that the two files belong together; it uses the pairing you
supplied. On a fresh destination, the stored layout is exactly the two sibling
files:

```text
seeds/imported/
  abc123.async.js
  abc123.raw.js
```

The original files under `downloads/` remain untouched. The kernel must classify
as a raw kernel before anything is written. Repeating the same bytes reports a
duplicate. The same token with different kernel bytes is kept beside the first
as a digest-suffixed variant, so distinct builds are not silently dropped.

When no launcher was saved, pass the identity directly. Exactly one of
`--launcher` or `--seed` is accepted:

```sh
shape seeds import downloads/abc123.raw.js \
  --seed abc123 --out seeds/imported
```

`--out` defaults to `seeds`. `--seed-param KEY` selects the query parameter that
carries the token when a launcher uses a name other than the default `seed`.

After import, trace the stored kernel like any other capture. The
[Build once](../../README.md#build-once) step already creates `reports/`; run
`mkdir -p reports` if you are following this example on its own:

```sh
shape trace program seeds/imported/abc123.raw.js \
  --out reports/program.js
```

Import arranges files for offline analysis. It cannot recover a launcher, kernel,
or bytes that were never saved. The `abc123` files above are a fictitious input
scenario, not a third tracked capture; the repository still tracks exactly the
two canonical full captures.

## Development

Run these commands from the repository root:

```bash
cargo fmt -- --check
cargo test --test seeds_acquisition --test seeds_adversarial --test seeds_cli
cargo test
cargo clippy --all-targets -- -D warnings
```

The acquisition tests preserve exact-byte storage, deduplication, variant, and
no-overwrite assertions, plus launcher classification, capture-token/URL extraction,
redirect and status handling, size/time limits, and process-level CLI behavior.
No test executes captured JavaScript or WASM.
