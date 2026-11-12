# Python

Python 3.9+ operation-oriented SDK over `libshape`, using standard-library
`ctypes`. Importing `shape` does not load native code. Named operation families
mirror the [Rust SDK](../rust/README.md); `Generator` remains available for older
applications. Python `int` retains exact signed/unsigned 64-bit JSON values.

## Install

From the repository root, build the optimized library with
`scripts/build-sdk.sh`, then install the wrapper:

```sh
uv venv .venv
uv pip install --python .venv/bin/python setuptools wheel
uv pip install --python .venv/bin/python --no-build-isolation ./sdk/python
```

Alternatively run recipes from the repository root with
`PYTHONPATH=sdk/python uv run --no-project python your_script.py`.
Set `SHAPE_LIBRARY` to the full path of a particular library if necessary.
Otherwise discovery checks `shape/lib/`, repository `target/release/`, then
`target/iter/`. `shape.library_path()` reports the selected file; a missing
library raises `ShapeError(code="library_not_found", ...)` on first use.
Use an optimized native build for expensive tracing and seed recovery.

## Load once, decode, explain, edit, encode and verify

This complete recipe uses only the committed public synthetic C fixtures. Run
it from the repository root. The artifact contains executable decode recipes;
its compact schema is descriptive and cannot replace it.

```python
from copy import deepcopy
from pathlib import Path
import json
import shape

fixtures = Path("sdk/c/tests/fixtures")
capture = json.loads((fixtures / "capture.json").read_text(encoding="utf-8"))
with shape.artifact.load(fixtures / "artifact.json") as loaded:
    # Parse and validate the artifact once, outside a stream of captures.
    for headers in [capture, capture]:
        decoded = shape.decode(loaded, headers)
        assert decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"] == 32
        print(decoded["positionalSignals"]["StorageAccessApiPresence"])
        assert decoded["positionalSignals"]["ConstantEmptyString"]["constant"] == ""

    detail = shape.explain(loaded, decoded, "DeviceMemory")
    assert detail["status"] == "decoded"
    print(detail["fields"])
    edited = deepcopy(decoded)  # Keep originalReport and undecoded channels.
    edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 8
    encoded = shape.encode(loaded, edited)
    verified = shape.decode(loaded, encoded, verify=True)
    assert verified["positionalSignals"]["DeviceMemory"]["deviceMemory"] == 8
    assert verified["metadata"]["exactReencode"]
    schema = shape.schema(loaded)
    assert schema["format"] == "shape-seed-schema/1"
```

### Keep a loaded artifact

Keep the loaded artifact outside capture loops, as in the context manager
above. Decode, explain, encode and schema reuse its immutable prepared state.
The context manager calls `close()`, which rejects new calls and waits for
active calls before releasing the handle.

Threads can share a loaded artifact; keep edited dictionaries separate for
each caller. Calls are synchronous and release the GIL during native work.
Using a closed artifact raises `invalid_request`.

Keep the full decoded payload, including `metadata.originalReport`, when
editing and re-encoding. Use matching seed/build, session and site ID; a session
override replaces the whole session and must be the same for decode, explain
and encode. It cannot make another build’s layout compatible.

`shape.encode_signals(loaded, signals)` accepts bare named fields when you
intentionally do not need to preserve retained original report channels.
`shape.encode(loaded, decoded)` preserves that full context. Neither operation
collects signals or acquires sources.

## Pass data and files

Ordinary document arguments accept dictionaries/lists or JSON `str`/`bytes`
containing the document. A string containing a filename is still treated as
contents. To read files, pass an explicit `Path` (or other `PathLike`) to
`artifact.load` or `kernel.load`, or read the file yourself first.

Document results are ordinary mutable Python dictionaries and lists with type
annotations. Options use snake_case; output documents keep their published
field names, such as `siteId` and `positionalSignals`. See the
[document types](shape/_types.py) for field definitions; `py.typed` ships
annotations for editors and type checkers.

Python JSON integers stay exact even above 2^53; never route them through
`float`. Requests reject nonfinite floats, malformed text and unsupported
values. See [exact packing](#pack-and-unpack-exact-values) for byte inputs and
values that need more than ordinary JSON.

## Raw seed to executable artifact

Provide three matching files from your own build/session: `seed.js` (raw seed),
`init.js` (initializer JavaScript), and `headers.json` (suffix-keyed capture
`{"siteId": "...", "headers": {...}}`). Unlike the fixture recipe above, this
workflow requires those actual inputs; arbitrary scripts are not a substitute.
Every variable is initialized here.

```python
from pathlib import Path
import json
import shape

raw = Path("seed.js").read_text(encoding="utf-8")
initializer = Path("init.js").read_text(encoding="utf-8")
capture = json.loads(Path("headers.json").read_text(encoding="utf-8"))
traced = shape.trace.program(
    raw, name="seed.js", cache=shape.CachePolicy.disabled(),
    thread_cleanup=True, no_cleanup=False, jobs=None,
)
if not traced["complete"]:
    raise RuntimeError(traced["stopped"])
program = traced["code"]
session = shape.session.from_initializer(initializer)
# Finders and compilation use traced source; presentation naming is optional.
contracts = shape.signals.contracts(program)
print(contracts)
with shape.artifact.compile(program, session, capture["siteId"]) as loaded:
    Path("decode-artifact.json").write_bytes(loaded.to_json())
    Path("schema.json").write_text(json.dumps(shape.schema(loaded)), encoding="utf-8")
    decoded = shape.decode(loaded, capture)
    print(decoded["positionalSignals"])
Path("program.js").write_text(program, encoding="utf-8")
```

Tracing defaults to parallel thread cleanup, full mechanical finishing
(`no_cleanup=False`), automatic worker count (`jobs=None`), quiet output and
no disk cache. Check `complete` and inspect `stopped` before using the program.

Mechanical finishing normalizes parameters/captures and substitutes kernel
values. Choose `no_cleanup=True` only when you want diagnostic output that
skips the full cleanup pipeline. Request semantic naming separately when its
source rewrites or role reports help your investigation.

## Explicit persistence and caching

Choose what to save for the work you want to repeat:

- Save an executable decode artifact to reuse its session, keys and compiled
  recipes with matching captures. `artifact.compile` returns it already loaded;
  export with `loaded.to_json()` and write the file yourself.
- Save a kernel artifact to reuse the compiled VM image with matching raw source.
- Save a traced program to skip tracing next time. A schema is for inspection
  and cannot replace an executable decode artifact.

Keeping an artifact loaded reuses memory. Disk kernel caching is a separate,
per-call choice:

| Policy | What it does |
| --- | --- |
| `CachePolicy.disabled()` (default) | No kernel cache reads or writes |
| `CachePolicy.directory(path)` | Reuse and publish kernels in the selected directory |
| `CachePolicy.environment()` | Explicitly use native cache environment settings |

The SDK never rewrites process environment variables. These policies do not
cache traced programs, ASTs or naming results; a kernel cache hit still runs
the trace.

```python
from pathlib import Path
import shape

raw = Path("seed.js").read_text(encoding="utf-8")
uncached = shape.trace.program(raw, cache=shape.CachePolicy.disabled())
cached = shape.trace.program(raw, cache=shape.CachePolicy.directory("kernel-cache"))
from_environment = shape.trace.program(raw, cache=shape.CachePolicy.environment())
for traced in (uncached, cached, from_environment):
    if not traced["complete"]:
        raise RuntimeError(traced["stopped"])

kernel = shape.report(raw)
Path("kernel.json").write_text(shape.kernel.to_json(kernel), encoding="utf-8")
with shape.kernel.load(Path("kernel.json")) as restored:
    traced = shape.trace.program_with_artifact(raw, restored)
if not traced["complete"]:
    raise RuntimeError(traced["stopped"])
# A supplied kernel is reused explicitly, with source identity validation.
with shape.artifact.load(Path("decode-artifact.json")) as loaded:
    Path("decode-artifact-copy.json").write_bytes(loaded.to_json())
```

Invalid or stale explicit cache entries, source mismatches and cache publication
failures are errors. Kernel reuse still needs its matching raw source.
Reusing a loaded executable artifact needs neither raw source nor cache lookup.
The legacy `trace.program(report_dir=...)` option selects a directory kernel
cache, not an output directory; do not combine it with `cache`.

## Optional naming and signal analysis

This standalone recipe requires the complete `program.js` saved above. Naming
is deliberately separate from the tracing/compilation path. Catalog cards
are static descriptions; they do not claim a capture contains a signal.

```python
from pathlib import Path
import shape

program = Path("program.js").read_text(encoding="utf-8")
print(shape.naming.threads(program))
print(shape.naming.collectors(program))
print(shape.naming.plan(program))
print(shape.naming.inventory(program))
print(shape.signals.catalog())
print(shape.signals.describe("DeviceMemory"))
print(shape.signals.schema(program))
print(shape.signals.show(program))
print(shape.signals.inputs(program))
print(shape.signals.contracts(program))
print(shape.signals.compact_schema(program))
```

Source analysis reparses/analyzes supplied source. Detailed signal schema,
compact schema, executable artifact and loaded native handle are distinct
outputs with distinct purposes.

## Source identity and explicit acquisition

### Read a saved launcher or kernel

Use `seeds.parse_launcher(data)` to read saved launcher bytes, then pass its
`kernel_ref` field to `seeds.resolve_url(reference, base)` with the captured
base URL. `seeds.parse(data)` identifies launcher or kernel bytes;
`seeds.parse_kernel(data)` summarizes a kernel.

The complete recipe below reads a local kernel and explicitly saves it to
`seed-store` with `import_seed`. Run with your matching `seed.js` and pass
its actual captured URL as the first script argument. The caller is responsible
for supplying the correct URL/token association.

```python
from pathlib import Path
import sys
import shape

if len(sys.argv) != 2:
    raise SystemExit("pass the matching captured kernel URL")
reference = shape.seeds.parse_url(sys.argv[1])
data = Path("seed.js").read_bytes()
classified = shape.seeds.parse(data)
if classified["kind"] != "kernel":
    raise ValueError("seed.js must be a kernel, not a launcher")
summary = shape.seeds.parse_kernel(data)
token = shape.seeds.token(reference, "seed")
stored = shape.seeds.import_seed("seed-store", token, data)
print(summary, stored["kernel_path"])
```

### Look up a listed site

Use `seeds.sites()` to list registry status or `seeds.site(name)` to look up
one entry. These calls do not fetch a seed.

### Fetch only when requested

`seeds.fetch(url, client=config)` acquires launcher/kernel inputs;
`seeds.fetch_direct(url, client=config)` fetches a kernel directly.

For bounded requests, create `config` with `seeds.client(...)`. Creating the
configuration does not send a request:

| Option | Controls |
| --- | --- |
| `timeout_ms` | Overall request timeout |
| `connect_timeout_ms` | Connection timeout |
| `max_body_bytes` | Response body size limit |
| `max_redirects` | Redirect limit |

See the [seed API](shape/seeds.py) for headers, base URL and token-parameter
options. If you use your own Python HTTP client, pass the downloaded bytes to
the parsing or import operations.

### Save fetched inputs explicitly

Fetching returns byte arrays without saving them. Call `seeds.import_seed`
with an explicit directory, token and kernel bytes to write a seed store, as
in the recipe above; optional launcher bytes can be saved with it.
Parsing and URL resolution do not write files.

## Header and payload framing

This complete synthetic recipe uses committed public profiles and generic report
channels, independent of named-field capture decoding. Full-name header maps
are distinct from suffix-keyed `CaptureHeaders`. Report decoding requires the
source-derived positional count and ordered keysets; arbitrary ciphertext
cannot provide that geometry.

```python
from pathlib import Path
import json
import shape

fixtures = Path("tests/fixtures/sdk/generator")
profile = json.loads((fixtures / "profile.json").read_text(encoding="utf-8"))
report = json.loads((fixtures / "report.json").read_text(encoding="utf-8"))
header_profile = shape.headers.codec(profile)
payload_profile = shape.payload.codec({key: profile[key] for key in ("format", "cipher", "alphabet", "keys")})
count = len(report["positionalSignals"])
keysets = report["keysets"]
text = shape.payload.encode(payload_profile, report)
recovered = shape.payload.decode(payload_profile, text, count, keysets)
assert shape.payload.encode(payload_profile, recovered) == text
assert recovered["seeds"] == report["seeds"]
headers = shape.headers.encode(header_profile, report)
parsed = shape.headers.parse("\n".join(f"{key}: {value}" for key, value in headers.items()))
assert parsed == headers
recovered_headers = shape.headers.decode(header_profile, headers, count, keysets)
assert shape.headers.encode(header_profile, recovered_headers) == headers
finalized = shape.headers.decode_finalized(header_profile, headers)
assert shape.headers.encode_finalized(header_profile, finalized["text"], finalized["context"]) == headers

plaintext = bytes([0, 255, 128, 65, 0])
text = shape.payload.encode_bytes(payload_profile, plaintext, [1, 2])
raw = shape.payload.decode_bytes(payload_profile, text)
assert bytes(raw["data"]["bytes"]) == plaintext
framed = shape.headers.encode_bytes(header_profile, raw)
recovered_raw = shape.headers.decode_bytes(header_profile, framed)
assert recovered_raw["seeds"] == raw["seeds"]
assert recovered_raw["data"] == raw["data"]
```

Report decoding canonicalizes object-entry order and may omit empty optional
fields; verify stable re-encoding rather than raw dictionary equality with an
uncanonical input document.

Framing encrypts/decrypts bytes; successful decoding does not establish signal
meaning or authenticity. Profiles are explicitly validated JSON values passed
per call, not hidden global codec state.

## Pack and unpack exact values

Superpack is independent of source analysis and encryption. This standalone
recipe uses tagged values to preserve binary/undefined/date distinctions and
IEEE-754 bits, including negative zero. Its number value is a 16-digit bit
string, not a JSON floating-point number.

```python
import shape

value = {"kind": "array", "value": [
    {"kind": "bool", "value": True},
    {"kind": "string", "value": "sample"},
    {"kind": "number", "value": "8000000000000000"},
    {"kind": "binary", "value": [0, 128, 255]},
]}
packed = shape.pack(value)
assert isinstance(packed, bytes)
assert shape.unpack_canonical(packed) == value
result = shape.unpack(packed)
assert result["canonical"]
assert bytes(result["original_bytes"]) == packed
```

Optional serializer options govern extensions, keysets, depth and resource
limits and must match between encoding and decoding. Plain JSON integers in
artifact/report documents remain arbitrary-precision Python `int`; do not
convert them through `float`. Byte inputs use `bytes`, `bytearray` or
`memoryview`; the wrapper converts them to the contract's u8 arrays.

## Advanced classification and corpus operations

This complete recipe uses the public synthetic VM fixture. Reachable
tracing reports each function's stop facts and differs from full corpus
convergence/self-check processing. Limits can leave functions unwalked.

```python
from pathlib import Path
import shape

seed_path = Path("tests/fixtures/javascript-vm-prefix.raw.js")
raw = seed_path.read_text(encoding="utf-8")
classification = shape.ops.classify.source(raw)
assert shape.ops.classify.report(raw) == classification
print(shape.ops.classify.seeds([classification]))
if classification["ops"]:
    handler = classification["ops"][0]["handler_index"]
    print(shape.ops.classify.template(raw, handler))

kernel_document = shape.trace.corpus.load(raw, cache=shape.CachePolicy.disabled())
with shape.kernel.load(kernel_document) as kernel:
    walked = shape.trace.corpus.reachable(kernel, steps=16, jobs=1)
    for function in walked:
        print(function["function"], function["print"])
walked_source = shape.trace.corpus.source(raw, steps=16, jobs=1)
print(walked_source)
full = shape.trace.corpus.walk_seed(seed_path, steps=16, self_check=True, cleanup=True)
print(shape.trace.corpus.compare({"public": full}, {"public": full}))
print(shape.trace.corpus.compare_inline({}, {}))
if walked:
    function = walked[0]["function"]
    print(shape.trace.corpus.fingerprint(
        kernel_document, function["pc"], function["state"],
        descriptor=function["descriptor"], steps=16,
    ))

sources = [{"name": "public", "source": "(function main(){return 1;})();"}]
analysis = shape.naming.corpus.analyze(sources)
print([(item["name"], item["plan"]["supported_roles"]) for item in analysis])
print(shape.naming.corpus.compare(sources, sources))
```

Classification can start from raw source or a saved analysis document:

- `ops.classify.report(report=document)` accepts a serialized pipeline report
  instead of reconstructing it from source.
- `ops.classify.template(source, template=document)` accepts a serialized
  handler template and its handler source. To select a handler from raw source,
  pass `handler_index` instead, as above.

Each saved-document choice excludes its corresponding reconstruction input.
Fingerprinting replays the selected kernel thread. Naming corpus comparisons
use explicit named source arrays, and corpus results contain arrays of function
records.

Only `walk_seed` reads a seed file; its optional `branches` path requests
output. Analysis/source calls do not scan directories. See the
[operation reference](../operations.json) for document formats and options.

## Operation coverage

| Family | Named Python operations |
| --- | --- |
| Discovery | `version_info`, `operation_commands` (v2), `version`, `commands` (legacy), `library_path`, `candidate_paths` |
| Source/kernel | `deob`, `report`, `kernel.show`, `kernel.load`, `kernel.to_json` |
| VM operations | `ops.list`, `ops.show`, `ops.classify.source/report/template/seeds` |
| Trace/cleanup | `trace.program`, `program_with_artifact`, `thread`, `cleanup.thread` |
| Trace corpus | `trace.corpus.load/source/reachable/walk_seed/fingerprint/compare/compare_inline` |
| Naming | `naming.threads/collectors/plan/inventory`, `naming.corpus.analyze/compare` |
| Signals | `signals.schema/show/inputs/contracts/catalog/describe/compact_schema` |
| Session/artifact | `session.from_initializer`, `pin`, `artifact.load/from_artifact/compile/to_json/compact_schema` |
| Loaded operations | `decode`, `explain`, `encode`, `encode_signals`, `schema` |
| Sources | `seeds.parse/parse_launcher/parse_kernel/parse_url/resolve_url/token/sites/site/client/fetch/fetch_direct/import_seed` |
| Headers | `headers.codec/parse/encode/decode/encode_bytes/decode_bytes/encode_finalized/decode_finalized` |
| Payload | `payload.codec/encode/decode/encode_bytes/decode_bytes` |
| Superpack | `pack`, `unpack`, `unpack_canonical` |

See the [operation reference](../operations.json) for requests, options and
defaults, and the [Python modules](shape/) for signatures.

## Handle errors

`ShapeError` provides a stable `code` for handling errors and a `message`
for diagnosis. Choose a response based on the code:

| Code | What to do |
| --- | --- |
| `invalid_request` | Check input types, document format, profiles and matching site/session/source; use executable artifacts rather than compact schemas |
| `unknown_command` | Check the operation name and installed SDK/native versions |
| `failed` | Read the diagnostic message and check the operation's input requirements |
| `panic` | Keep a reproducible input and report the native failure |
| `library_not_found` | Install a matching native release library or set `SHAPE_LIBRARY` to its full path |
| `unsupported_library` | Rebuild/install the native library matching this wrapper; the selected library lacks a required operation |

Malformed profiles, mismatched inputs and unsupported edits are errors.
Check trace completion and use `verify=True` when checking exact re-encoding.

## Tests

See [the test guide](tests/README.md) for unit, runtime, operation coverage and
README recipe checks using public fixtures and an explicit provider-built
release library.
