<a id="shape--c-sdk-abi"></a>

# C

Use `libshape` to recover captured scripts, inspect signals, and decode or
encode headers. Operations take JSON text and return JSON text; load functions
take bytes and a length. Release library result strings with `shape_free`.

Choose the task that matches your input:

- With raw seed, initializer and headers, follow the [workflow](#workflow).
- With a decode artifact, [load once and decode many captures](#a-standalone-loaded-artifact-workflow).
- For direct source analysis and other operations, use `shape_call_v2` and the
  [operation reference](../operations.json). Disk caching is disabled unless
  you enable it. Legacy `shape_call` keeps its older defaults.

The [shared SDK guide](../README.md) explains matching inputs and document
formats; the [signal editing walkthrough](../signals.md) shows field values.
The public header, [`include/shape.h`](include/shape.h), details pointer and
ownership requirements. Python, Node.js and Go wrap this C interface; the
[Rust SDK](../rust/README.md) calls the engine directly.

**Contents:** [Install and build](#install-and-build) · [Workflow](#workflow) ·
[Decode, explain and encode](#decode-explain-and-encode)
([generator methods](#generator-methods)) ·
[API](#api) ([requests](#requests), [stateless commands](#stateless-commands)) · [Precision](#precision) ·
[Errors](#errors) · [Threading and lifetime](#threading-and-lifetime) ·
[Performance](#performance) · [Troubleshooting](#troubleshooting) ·
[Tests](#tests)

## Install and build

From the repository root:

Requirements:

| Requirement | Why |
| --- | --- |
| Rust 1.92+ and Cargo | Builds the `shape-c` crate |
| A C compiler and toolchain | Compiles and links your program |
| macOS or Linux | The helper script's install layout; Windows can build with Cargo but needs its own packaging |
| A JSON library or manual JSON I/O | The ABI exchanges JSON text, not structs |

```sh
scripts/build-sdk.sh
```

The script builds the release `shape-c` library with Cargo and, by default,
leaves the outputs in `target/release/`:

| Platform | Shared library | Static library |
| --- | --- | --- |
| macOS | `target/release/libshape.dylib` | `target/release/libshape.a` |
| Linux | `target/release/libshape.so` | `target/release/libshape.a` |

`scripts/build-sdk.sh` supports macOS and Linux. On Windows, build `shape-c`
with Cargo; the dynamic library is `shape.dll`, linked with the import library
your toolchain expects.

It also refreshes the native library copies used by the Python, Node.js and Go
bindings, and rebuilds the Node addon. It is safe to rerun, and its outputs stay
Git-ignored. Building requires Rust 1.92 or newer; compiling your own C program
requires a C compiler and toolchain.

If you only need the C library, build it without installing the binding copies.
On Linux, `cargo build --release -p shape-c` produces the same two files. On
macOS, build the relocatable, linkable form directly (see the note below):

```sh
cargo rustc --release -p shape-c -- \
  -C strip=none \
  -C link-arg=-Wl,-install_name,@rpath/libshape.dylib
```

macOS note: a *stripped* Rust cdylib on current toolchains has an unaligned
`LINKEDIT` string table, and the system linker then refuses to link against it
(`mis-aligned LINKEDIT string pool`). `-C strip=none` avoids that, and the
install name makes the dylib relocatable. Do not run `install_name_tool -id` on
it: rewriting the load commands reproduces the alignment problem.

### Link

The header lives in `sdk/c/include` and the library in `target/release`. Link
the shared library with `-lshape`:

```sh
# macOS
cc app.c -I sdk/c/include -L target/release -lshape \
  -Wl,-rpath,@loader_path/target/release -o app

# Linux
cc app.c -I sdk/c/include -L target/release -lshape \
  -Wl,-rpath,'$ORIGIN/target/release' -o app
```

`@loader_path` (macOS) and `$ORIGIN` (Linux) are relative to the executable, so
these commands assume `app` is in the repository root. If it is elsewhere, use
the correct relative path, an absolute rpath, or copy the library next to the
executable. On macOS the build script sets the install name to
`@rpath/libshape.dylib`; the rpath above makes that name resolvable.

To link statically, use the archive and the system libraries the Rust runtime
needs:

```sh
# macOS
cc app.c -I sdk/c/include target/release/libshape.a \
  -framework CoreFoundation -framework Security -o app

# Linux
cc app.c -I sdk/c/include target/release/libshape.a \
  -ldl -lpthread -lm -lrt -o app
```

The header has `extern "C"` guards, so C++ callers can include it directly.

## Workflow

The full path from a captured seed to decoded and re-encoded headers. The
[shared workflow](../README.md#workflow) explains each step and document.

You supply three matching inputs from the same build and session:

| File | Contents |
| --- | --- |
| `seed.js` | Raw VM seed text |
| `init.js` | The matching initializer JavaScript |
| `headers.json` | The captured request as `{"siteId":"my-site","headers":{"a":"…"}}` |

Header names are suffixes, including numeric chunks such as `a0`; the decode
artifact supplies their shared prefix. These are your own inputs, not generated
placeholder files.

The program below does four things:

1. Traces the seed once and checks `complete`.
2. Extracts the session from the initializer.
3. Compiles directly to a validated handle, then deliberately exports it to
   `artifact.json`. Later runs may
   load that file directly; this run decodes with the original compiled handle.
   Repeated decoding needs only it and matching
   capture headers.
4. Decodes the headers, explains `DeviceMemory`, and encodes the payload back.
   Choose a signal present in your capture if it has no `DeviceMemory`.

Save it as `workflow.c` in the repository root, compile it with the flags from
[Link](#link), and run it with the capture's actual site ID:

```sh
cc workflow.c -I sdk/c/include -L target/release -lshape \
  -Wl,-rpath,@loader_path/target/release -o app      # macOS; see Link for Linux
./app seed.js init.js headers.json my-site
```

The small JSON helpers in it only extract values from library-produced JSON and
preserve integer tokens as text. A general application can use a lossless JSON
library instead.

```c
#include "shape.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *read_file(const char *path, size_t *length) {
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        perror(path);
        exit(1);
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        perror(path);
        exit(1);
    }
    long size = ftell(file);
    rewind(file);
    if (size < 0) {
        perror(path);
        exit(1);
    }
    char *data = malloc((size_t)size + 1);
    if (data == NULL || fread(data, 1, (size_t)size, file) != (size_t)size) {
        perror(path);
        exit(1);
    }
    fclose(file);
    data[size] = '\0';
    if (length != NULL) {
        *length = (size_t)size;
    }
    return data;
}

static char *request_field(const char *field, const char *json,
                           const char *extra) {
    size_t size = strlen(field) + strlen(json) + 8;
    if (extra != NULL) {
        size += strlen(extra) + 1;
    }
    char *text = malloc(size);
    if (text == NULL) {
        exit(1);
    }
    snprintf(text, size, "{\"%s\":%s%s%s}", field, json,
             extra == NULL ? "" : ",", extra == NULL ? "" : extra);
    return text;
}

/* Quote UTF-8 source as JSON without interpreting any numeric document tokens. */
static char *quote_json(const char *text) {
    size_t length = strlen(text);
    char *json = malloc(length * 6 + 3);
    if (json == NULL) exit(1);
    char *out = json;
    *out++ = '"';
    for (const unsigned char *p = (const unsigned char *)text; *p; ++p) {
        if (*p == '"' || *p == '\\') { *out++ = '\\'; *out++ = (char)*p; }
        else if (*p < 32) { snprintf(out, 7, "\\u%04x", *p); out += 6; }
        else *out++ = (char)*p;
    }
    *out++ = '"';
    *out = '\0';
    return json;
}

/* Copy a top-level JSON value as text. Input is valid library-produced JSON;
 * nested strings/containers are skipped without parsing integers as doubles. */
static char *field_json(const char *json, const char *key) {
    const char *p = json + 1;
    while (*p && *p != '}') {
        while (*p == ' ' || *p == '\n' || *p == ',' || *p == '\r' || *p == '\t') ++p;
        if (*p != '"') break;
        const char *name = ++p;
        while (*p && *p != '"') {
            if (*p == '\\' && p[1]) ++p;
            ++p;
        }
        size_t name_length = (size_t)(p - name);
        if (*p != '"') break;
        ++p;
        while (*p == ' ' || *p == ':') ++p;
        const char *start = p;
        int depth = 0, string = 0;
        for (; *p; ++p) {
            if (string) {
                if (*p == '\\' && p[1]) ++p;
                else if (*p == '"') string = 0;
            } else {
                if (depth == 0 && (*p == ',' || *p == '}')) break;
                if (*p == '"') string = 1;
                else if (*p == '{' || *p == '[') ++depth;
                else if (*p == '}' || *p == ']') --depth;
            }
        }
        if (strlen(key) == name_length && memcmp(name, key, name_length) == 0) {
            size_t length = (size_t)(p - start);
            char *value = malloc(length + 1);
            if (value == NULL) exit(1);
            memcpy(value, start, length);
            value[length] = '\0';
            return value;
        }
    }
    fprintf(stderr, "missing JSON field: %s\n", key);
    exit(1);
}

/* Free the library envelope after copying its value; propagate every error. */
static char *value_of(char *envelope) {
    if (envelope == NULL) { fprintf(stderr, "null response\n"); exit(1); }
    char *ok = field_json(envelope, "ok");
    if (strcmp(ok, "true") != 0) {
        fprintf(stderr, "shape call failed: %s\n", envelope);
        free(ok);
        shape_free(envelope);
        exit(1);
    }
    free(ok);
    char *value = field_json(envelope, "value");
    shape_free(envelope);
    return value;
}

int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr, "usage: %s seed.js init.js headers.json site-id\n", argv[0]);
        return 2;
    }
    char *raw_seed = read_file(argv[1], NULL);
    char *source_json = quote_json(raw_seed);
    free(raw_seed);
    char *trace_request = request_field("source", source_json, "\"cache\":{\"mode\":\"disabled\"}");
    free(source_json);
    char *traced = value_of(shape_call_v2("trace.program", trace_request));
    free(trace_request);
    char *complete = field_json(traced, "complete");
    if (strcmp(complete, "true") != 0) {
        fprintf(stderr, "incomplete trace: %s\n", traced);
        free(complete);
        free(traced);
        return 1;
    }
    free(complete);
    char *program_json = field_json(traced, "code"); /* JSON string literal */
    char *schema_request = request_field("source", program_json, NULL);
    char *signal_schema = value_of(shape_call_v2("signals.schema", schema_request));
    free(schema_request);
    printf("signal schema: %s\n", signal_schema);
    free(signal_schema);
    free(traced);
    char *initializer = read_file(argv[2], NULL);
    char *initializer_json = quote_json(initializer);
    free(initializer);
    char *session_request = request_field("init", initializer_json, NULL);
    free(initializer_json);
    char *session = value_of(shape_call_v2("session.from_initializer", session_request));
    free(session_request);
    char *site_id = quote_json(argv[4]);
    size_t extra_length = strlen(session) + strlen(site_id) + 32;
    char *extra = malloc(extra_length);
    if (extra == NULL) exit(1);
    snprintf(extra, extra_length, "\"session\":%s,\"site_id\":%s", session, site_id);
    free(session);
    free(site_id);
    char *compile_request = request_field("source", program_json, extra);
    free(program_json);
    free(extra);
    char *error = NULL;
    ShapeArtifact *artifact_handle = shape_artifact_compile(compile_request, &error);
    free(compile_request);
    if (artifact_handle == NULL) {
        fprintf(stderr, "compile failed: %s\n", error == NULL ? "(no message)" : error);
        shape_free(error);
        return 1;
    }

    /* Export deliberately for persistence; decoding already has a live handle. */
    char *artifact = value_of(shape_artifact_call(artifact_handle, "artifact", NULL));
    FILE *saved = fopen("artifact.json", "wb");
    if (saved == NULL) { perror("artifact.json"); return 1; }
    size_t artifact_length = strlen(artifact);
    if (fwrite(artifact, 1, artifact_length, saved) != artifact_length) return 1;
    if (fclose(saved) != 0) return 1;
    free(artifact);

    char *headers_json = read_file(argv[3], NULL);
    char *decode_request = request_field("capture", headers_json, NULL);
    free(headers_json);
    char *decoded = value_of(shape_decode(artifact_handle, decode_request));
    free(decode_request);

    char *explain_request =
        request_field("decoded", decoded, "\"signal\":\"DeviceMemory\"");
    char *signal = value_of(shape_explain(artifact_handle, explain_request));
    free(explain_request);
    printf("DeviceMemory: %s\n", signal);
    free(signal);

    /* Default decode retained originalReport, so full-payload encode reuses bytes. */
    char *encode_request = request_field("decoded", decoded, NULL);
    char *encoded = value_of(shape_encode(artifact_handle, encode_request));
    free(encode_request);
    printf("headers: %s\n", encoded);

    free(encoded);
    free(decoded);
    shape_artifact_free(artifact_handle);
    return 0;
}
```

Library strings use `shape_free`; copies and requests allocated by these helpers
use `free`. Keep each handle alive until all calls using it have returned.

## Decode, explain and encode

The [decoded signals and editing walkthrough](../signals.md) shows concrete
output and a named-field round trip. The
[shared contract](../README.md#decode-explain-and-encode) holds for every SDK;
this section is the C view of it, followed by the full method reference.

### Decode options

`decode` defaults to `verify:false` and `metadata:false`. It returns named
fields in `positionalSignals` and **always** retains
`metadata.originalReport`, including raw signal bytes.

| Option | Effect when `true` |
| --- | --- |
| `metadata` | Also builds detailed `metadata.signals` for every signal. Without it, `explain` builds one signal's details on demand from the retained report. |
| `verify` | Independently checks byte-identical signal and header re-encoding. `metadata.exactReencode` records whether that check ran. |

### Encode inputs

Pass exactly one of `decoded` or `signals`.

| Request | Result |
| --- | --- |
| `decoded`: the full payload, unchanged | Reuses channels, seeds, keysets, header context and per-signal random prefixes. This preserves the captured header values when the artifact and session match and the input has a supported encoding. It works after a default decode; `metadata:true` is not required. |
| `decoded`: the full payload, with edited fields | Encodes the edited values. They must decode back exactly, or encoding fails. |
| `signals`: a bare `positionalSignals` map | Uses fresh random prefixes and layout defaults for the other channels. It does not promise the original bytes. |

Encoding performs its own field round-trip check regardless of the decode
`verify` option.

### Sessions

The artifact's session is the default. A supplied `session` replaces it as a
whole. Pass the same replacement to decode, explain and encode. The compact
schema is descriptive and cannot be loaded as a decode artifact.

### A standalone loaded-artifact workflow

Save as `loaded_workflow.c` or compile the identical checked-in
[`tests/loaded_workflow.c`](tests/loaded_workflow.c). It needs no helper code
from another example. The checked-in synthetic fixture includes DeviceMemory.
The raw-source example above is also available as [`tests/workflow.c`](tests/workflow.c).

```sh
LIB="${CARGO_TARGET_DIR:-$PWD/target}/release"
cc -std=c99 -Wall -Wextra -Werror -pedantic -I sdk/c/include \
  sdk/c/tests/loaded_workflow.c -L "$LIB" -lshape -Wl,-rpath,"$LIB" -o /tmp/loaded-workflow
/tmp/loaded-workflow sdk/c/tests/fixtures/artifact.json sdk/c/tests/fixtures/capture.json
cc -std=c99 -Wall -Wextra -Werror -pedantic -I sdk/c/include \
  sdk/c/tests/workflow.c -L "$LIB" -lshape -Wl,-rpath,"$LIB" -o /tmp/raw-workflow
# Matching raw capture inputs from the same build and session:
/tmp/raw-workflow seed.js init.js headers.json my-site
```

```c
#include "shape.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *read_file(const char *path, size_t *length) {
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        perror(path);
        exit(1);
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        perror(path);
        exit(1);
    }
    long size = ftell(file);
    rewind(file);
    if (size < 0) {
        perror(path);
        exit(1);
    }
    char *data = malloc((size_t)size + 1);
    if (data == NULL || fread(data, 1, (size_t)size, file) != (size_t)size) {
        perror(path);
        exit(1);
    }
    fclose(file);
    data[size] = '\0';
    if (length != NULL) {
        *length = (size_t)size;
    }
    return data;
}

static char *request_field(const char *field, const char *json,
                           const char *extra) {
    size_t size = strlen(field) + strlen(json) + 8;
    if (extra != NULL) {
        size += strlen(extra) + 1;
    }
    char *text = malloc(size);
    if (text == NULL) {
        exit(1);
    }
    snprintf(text, size, "{\"%s\":%s%s%s}", field, json,
             extra == NULL ? "" : ",", extra == NULL ? "" : extra);
    return text;
}

/* Copy a top-level JSON value as text. Input is valid library-produced JSON;
 * nested strings/containers are skipped without parsing integers as doubles. */
static char *field_json(const char *json, const char *key) {
    const char *p = json + 1;
    while (*p && *p != '}') {
        while (*p == ' ' || *p == '\n' || *p == ',' || *p == '\r' || *p == '\t') ++p;
        if (*p != '"') break;
        const char *name = ++p;
        while (*p && *p != '"') {
            if (*p == '\\' && p[1]) ++p;
            ++p;
        }
        size_t name_length = (size_t)(p - name);
        if (*p != '"') break;
        ++p;
        while (*p == ' ' || *p == ':') ++p;
        const char *start = p;
        int depth = 0, string = 0;
        for (; *p; ++p) {
            if (string) {
                if (*p == '\\' && p[1]) ++p;
                else if (*p == '"') string = 0;
            } else {
                if (depth == 0 && (*p == ',' || *p == '}')) break;
                if (*p == '"') string = 1;
                else if (*p == '{' || *p == '[') ++depth;
                else if (*p == '}' || *p == ']') --depth;
            }
        }
        if (strlen(key) == name_length && memcmp(name, key, name_length) == 0) {
            size_t length = (size_t)(p - start);
            char *value = malloc(length + 1);
            if (value == NULL) exit(1);
            memcpy(value, start, length);
            value[length] = '\0';
            return value;
        }
    }
    fprintf(stderr, "missing JSON field: %s\n", key);
    exit(1);
}

/* Free the library envelope after copying its value; propagate every error. */
static char *value_of(char *envelope) {
    if (envelope == NULL) { fprintf(stderr, "null response\n"); exit(1); }
    char *ok = field_json(envelope, "ok");
    if (strcmp(ok, "true") != 0) {
        fprintf(stderr, "shape call failed: %s\n", envelope);
        free(ok);
        shape_free(envelope);
        exit(1);
    }
    free(ok);
    char *value = field_json(envelope, "value");
    shape_free(envelope);
    return value;
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: %s artifact.json capture.json\n", argv[0]);
        return 2;
    }
    size_t length;
    char *bytes = read_file(argv[1], &length);
    char *error = NULL;
    ShapeArtifact *artifact = shape_artifact_load((const uint8_t *)bytes, length, &error);
    free(bytes);
    if (artifact == NULL) {
        fprintf(stderr, "%s\n", error ? error : "load failed");
        shape_free(error);
        return 1;
    }
    char *schema = value_of(shape_schema(artifact, "{}"));
    printf("schema: %s\n", schema);
    free(schema);
    char *capture = read_file(argv[2], NULL);
    char *request = request_field("capture", capture, NULL);
    free(capture);
    char *decoded = value_of(shape_decode(artifact, request));
    free(request);
    request = request_field("decoded", decoded, "\"signal\":\"DeviceMemory\"");
    char *detail = value_of(shape_explain(artifact, request));
    free(request);
    printf("DeviceMemory: %s\n", detail);
    free(detail);
    request = request_field("decoded", decoded, NULL);
    char *encoded = value_of(shape_encode(artifact, request));
    free(request);
    printf("unchanged headers: %s\n", encoded);
    free(encoded);

    /* Explicit named-signal edit, suitable for this synthetic fixture. For a
     * capture edit the full decoded document with a lossless JSON parser to
     * retain seeds, prefixes, sibling channels and metadata.originalReport. */
    encoded = value_of(shape_encode(artifact,
        "{\"signals\":{\"DeviceMemory\":{\"deviceMemory\":16}}}"));
    request = request_field("capture", encoded, NULL);
    char *edited = value_of(shape_decode(artifact, request));
    free(request);
    printf("edited payload: %s\n", edited);
    free(edited);
    free(encoded);
    free(decoded);
    shape_artifact_free(artifact);
    return 0;
}

```

### Reuse handles and choose caching

The loaded artifact parses and validates once. Direct calls reuse it without
reloading JSON or reading disk cache. Use
`shape_artifact_call(handle, "artifact", NULL)` to export it deliberately.

`shape_kernel_load/call/free` reuse a compiled kernel for `trace.program`,
`trace.thread`, `trace.corpus.reachable` and `kernel.to_json`. Program tracing
still needs matching raw source. Loaded kernel calls validate a supplied cache
policy but do not read disk cache.

For named source operations, choose caching per request:

| `cache` value | Effect |
| --- | --- |
| `{"mode":"disabled"}` (default) | No disk-cache reads or writes |
| `{"mode":"directory","directory":"cache"}` | Use the explicit directory |
| `{"mode":"environment"}` | Opt into environment/default cache resolution |

No call mutates the process environment. For `shape_call_v2` tracing,
`thread_cleanup=true`, `no_cleanup=false` and `jobs=null` keep full mechanical
finishing and use available cores. Naming is optional; `no_cleanup=true`
requests diagnostic output with the lighter cleanup.

`seeds.fetch` and `seeds.fetch_direct` download scripts; `seeds.import` writes
to a chosen store.
See the [operation reference](../operations.json) for their inputs and limits.

<a id="compatibility-generator-methods"></a>

### Generator methods

Use `shape_generator_call` for generator decode/explain/encode/schema methods.

Load an artifact once, then decode many captures:

```c
size_t len;
char *artifact = read_file("artifact.json", &len);
char *error = NULL;
ShapeGenerator *generator =
    shape_generator_load((const uint8_t *)artifact, len, &error);
if (generator == NULL) { /* error is an envelope; free it with shape_free */ }
free(artifact);
```

`shape_generator_load` validates the artifact once. A generator is immutable
and may be shared by any number of threads. Call
`shape_generator_call(generator, method, request_json)` for:

| Method | Request fields | Result `value` |
| --- | --- | --- |
| `decode` | `capture` document, required; `session` document; `verify` bool, default `false`; `metadata` bool, default `false` | decoded payload |
| `explain` | `decoded` document, required; `signal` string, required; `session` document; `verify` bool, default `false` | one named signal |
| `encode` | exactly one of `decoded` (full decoded payload) or `signals` (a bare `positionalSignals` map); `session` document | `CaptureHeaders` |
| `schema` | none | compact schema |

`capture` is `{"siteId":string,"headers":{suffix:string}}`. Header keys are
suffixes `a` through `z` or numeric chunks such as `a0`; the artifact supplies
the shared prefix, and `siteId` must equal the artifact's `siteId`. Header
values are strings. `session` replaces the artifact's session as a whole; do
not mix values from two captures.

`decode` returns the slim payload:

```json
{
  "siteId": "…",
  "errors": [],
  "positionalSignals": {"DeviceMemory": {"deviceMemory": 9}},
  "signals": [],
  "keyedSignals": [],
  "seeds": [0, 0],
  "keysets": [],
  "metadata": {
    "originalReport": {},
    "signals": {},
    "decoded": 1,
    "missing": 0,
    "exactReencode": false,
    "floatByteOrder": "little"
  }
}
```

`metadata.originalReport` retains the raw report channels, including each
signal's bytes, and is always present. `metadata.signals` is empty unless you
ask for `metadata:true`; use `explain` to build one entry later.
`metadata.exactReencode` reports whether `verify` ran. Without `metadata`,
only the named field objects are built.

`explain` takes the decoded payload (its retained original report is what
supplies the raw signal bytes) and returns one signal:

```json
{
  "fields": {"deviceMemory": 9},
  "status": "decoded",
  "metadata": {}
}
```

`fields` may be null for a non-decoded status. `metadata` carries the leaves,
occurrences, raw bytes and flags used for that projection. Explaining a signal
that is absent from the payload, or naming one the artifact does not route, is
an error.

`encode` returns a capture document with `siteId` and suffix-keyed `headers`.
Pass exactly one of these inputs, as an object or JSON text string:

- `decoded`: keep the full payload to retain channels, seeds, keysets, header
  context and each signal's four-byte random prefix.
- `signals`: pass a bare `positionalSignals` map to construct fields with fresh
  prefixes and layout defaults for other channels.

The encoder returns headers only if decoding them reproduces the edited fields
exactly. An unsupported edit fails instead of silently changing values.

`schema` returns the same compact schema as `artifact.compact_schema`. An
unknown method is `unknown_command`.

## API

### Choose an entry point

| Task | Function |
| --- | --- |
| Run a source or standalone operation | `shape_call_v2(operation, request_json)` |
| Compile or load executable decode recipes | `shape_artifact_compile` / `shape_artifact_load` |
| Decode, encode, explain or inspect schema | `shape_decode`, `shape_encode`, `shape_explain`, `shape_schema` |
| Export a loaded decode artifact | `shape_artifact_call(handle, "artifact", NULL)` |
| Reuse a compiled kernel | `shape_kernel_load`, `shape_kernel_call` |
| Maintain an existing integration | `shape_call`, `shape_generator_load/call` |

Request/result details for named operations are in the
[operation reference](../operations.json). The legacy reference below retains
its own defaults. See [ownership](#ownership) before releasing any result.

### Requests

A request is a JSON object. Field names are the snake_case names in
the operation reference (`site_id`, `float_byte_order`, `thread_cleanup`), not the
camelCase names some command results use. Absent fields and fields set to
`null` take the documented default, except `artifact.compile.direct` must
be a boolean when present. The top-level request is read field by
field, so an unrelated field is harmless; a nested typed document such as
`capture`, `session` or `artifact` validates its own shape.

`source` is always text: a raw capture, traced program JavaScript, or another
JavaScript document, depending on the command. Other document arguments
(`capture`, `session`, `artifact`, `decoded`, `labels`, `signals`) may be JSON
objects or JSON text strings. These are document contents, not filenames.
Document fields do not load files; path/directory fields select explicit I/O.

### Branch overrides

Commands that walk a VM thread take an optional `overrides` object:

```json
{
  "fake": {"<pc>/<state>": true},
  "real": ["<pc>/<state>"]
}
```

`fake` marks a branch position as fake and gives the side always taken:
`true` takes the jump, `false` takes the fall-through. `real` marks positions
as real code. Positions are `"<pc>/<state>"` strings. Both members are
optional; the default is no overrides.

<a id="stateless-commands"></a>

### Legacy stateless commands

Call `shape_call(command, request_json)`. Every result below is the `value`
inside a success envelope.

##### Inspect source and kernel operations

| Command | Request fields | Result `value` |
| --- | --- | --- |
| `version` | none | `{"version":string,"commands":[string]}` |
| `deob` | `source` string, required; `artifact` bool, default `false` | `{"code":string}`; with `artifact:true` also `"artifact"` (kernel artifact) |
| `report` | `source` string, required; `input_kind` `"raw"` (default) or `"deobfuscated"` | kernel artifact |
| `kernel.show` | `source` string, required; `input_kind` `"raw"` (default) or `"deobfuscated"`; `operations` array of table-key strings, default `[]` | flat VM-properties document |
| `ops.list` | `source` string, required | operation catalogue |
| `ops.show` | `source` string, required; `op` non-negative `u32` operation table key, required | one operation table entry |

##### Trace and clean code

| Command | Request fields | Result `value` |
| --- | --- | --- |
| `trace.program` | `source` string, required; `name` string, default `"seed.js"`; `steps` integer, default `1000000`; `report_dir` string; `thread_cleanup` bool, default `false`; `no_cleanup` bool, default `false`; `check_reuse` bool, default `false`; `overrides` object; `jobs` integer; `quiet` bool, default `true` | linked-program summary |
| `trace.thread` | exactly one of `source` or `artifact`; `pc` integer, required; `state` integer, required; `descriptor` integer; `initial_slots` array of integers; `inputs` object; `steps` integer, default `1000000`; `self_check` bool, default `false`; `outline` bool, default `true`; `reverse_sides` bool, default `false`; `partial` bool, default `false`; `overrides` object | thread walk summary |
| `cleanup.thread` | `source` string, required | cleaned code and cleanup counters |

##### Investigate names

| Command | Request fields | Result `value` |
| --- | --- | --- |
| `naming.threads` | `source` string, required; `rename` bool, default `false`; `labels` label-reference document | `{"source":string,"report":object}` |
| `naming.collectors` | `source` string, required; `rename` bool, default `false` | `{"source":string,"report":object}` |
| `naming.plan` | `source` string, required | semantic naming plan |
| `naming.inventory` | `source` string, required | `{"function_index":object,"role_graph":object}` |

##### Inspect signals

| Command | Request fields | Result `value` |
| --- | --- | --- |
| `signals.schema` | `source` string, required; `name` string; `layouts_only` bool, default `false`; `finder_roles` bool, default `true` | per-seed signal schema |
| `signals.show` | `source` string, required | program-properties report |
| `signals.inputs` | `source` string, required | program input/context report |
| `signals.contracts` | `source` string, required | signal contract report |

##### Build session layouts and artifacts

| Command | Request fields | Result `value` |
| --- | --- | --- |
| `pin` | `source` string, required; exactly one of `session` or `init` | seed layout |
| `session.from_initializer` | `init` string, required | session document |
| `artifact.compile` | `source` string, required; `session` document, required; `site_id` string, required; `float_byte_order` optional `"little"` (default) or `"big"`; `input_kind` `"program"` (default) or `"raw"`; `name` string, default `"seed.js"` (names a raw seed trace); `direct` boolean, default false (source-format, non-lifecycle mode) | decode artifact |
| `artifact.compact_schema` | `artifact` document, required | compact schema |

`deob` with `artifact:true` compiles the kernel artifact from the raw source;
`report` returns the kernel artifact itself. `kernel.show` deobfuscates raw
input unless `input_kind` is `"deobfuscated"`; `operations` selects handlers by
their operation table key. Unknown keys are `invalid_request`.

#### Kernel artifact

`report` (and `deob` with `artifact:true`) returns the source-free kernel
artifact:

```json
{
  "format": "shape-kernel-artifact/v1",
  "compiler_provenance": "…",
  "source_hash": "…",
  "artifact_source_hash": "…",
  "source_basis": "…",
  "standardized_length": 0,
  "integrity_digest": "…",
  "payload": "base64 zstd-compressed JSON"
}
```

The artifact is the reusable input to `trace.thread` (`"artifact": …`) and to
the CLI's saved-artifact mode. It is not a decode artifact.

#### VM properties

`kernel.show` returns the flat tracer-input document: `entry` when known, one
key per operation table (`{key: handler source}`), the string table,
descriptor table, `vmContext`, `vmMemory`, `vmStack`, `bytecode`, module bytes,
memory limits, dispatch table, sample constants and the worker bootstrap.
Empty optional sections are omitted. The document is sample configuration
extracted from the source, not a closure-complete runtime dump.

#### Operations

`ops.list` returns the operation catalogue:
`{"capture":…,"derived_from":…,"table":…,"inventory":…,"handlers":[…]}`.
`ops.show` adds `handler`, `span_source`, `analyses`, and
`analyst_annotation` (null from the SDK). `op` is the operation table key shown
by `ops.list`.

#### Tracing

`trace.program` links the reachable program from a raw capture:

```json
{
  "code": "linked JavaScript",
  "complete": true,
  "traced": 0,
  "stopped": [{"function": "…", "reason": "…"}],
  "kernel_globals": ["…"]
}
```

Check `complete` before compiling or using the program. It is true exactly
when `stopped` is empty. For legacy `shape_call` tracing:

- `steps` bounds each function walk; `jobs` bounds concurrent walks (`null` uses every core).
- `report_dir` selects the kernel-artifact cache directory.
- `quiet` defaults to `true`; `name` labels diagnostics and cache entries.

`trace.thread` walks one VM thread of a kernel artifact:

```json
{
  "code": "emitted function or null",
  "complete": true,
  "stop": "reason code",
  "steps": 0,
  "handlers_traced": 0,
  "undeclared": ["…"],
  "self_check": ["…"],
  "source_hash": "…"
}
```

A stopped walk has null `code` unless `partial` is true. Partial output is for
investigation; it does not make the trace complete.

- `initial_slots` replaces the selected descriptor's slots.
- `inputs` supplies an explicit snapshot: slots, memory, stack, arguments, receiver and self.
- `self_check` records shortcut disagreements; an empty array means the check held.

`cleanup.thread` cleans one emitted function:

```json
{
  "code": "cleaned JavaScript",
  "cleanup": {
    "inlined_bindings": 0,
    "inlined_snapshots": 0,
    "rounds": 0,
    "inlined_helpers": 0,
    "removed_helpers": 0,
    "remaining_helpers": 0,
    "property_writes": 0,
    "arrays": 0,
    "methods": 0,
    "loops": 0,
    "flattened_blocks": 0
  }
}
```

#### Naming

`naming.threads` and `naming.collectors` return `{"source", "report"}`. With
`rename:false` (the default) `source` is unchanged and the report records
report-only decisions; with `rename:true` the accepted renames are applied.
`naming.threads` also accepts a saved `labels` document to inherit exact
structural names across an equal build.

| Investigation | Operation | What to inspect |
| --- | --- | --- |
| Review proposed names | `naming.plan` | Candidates, ambiguity, skipped/unproven roles; does not edit source |
| Explore relationships | `naming.inventory` | Function index and role graph, including unresolved calls and diagnostics |

#### Signals

Choose a report for the question you are investigating:

| Question | Operation | Result |
| --- | --- | --- |
| Which fields and encodings are supported? | `signals.schema` | Per-seed schema and diagnostics; `layouts_only` adds layouts, `finder_roles` enables finders |
| What does the program collect? | `signals.show` | Properties, probes, constants and unresolved context |
| Which external inputs does it use? | `signals.inputs` | Joined input and role/context view |
| How are signals extracted? | `signals.contracts` | Producer contracts, coverage and limitations; not decoded values |

See the [operation reference](../operations.json) for full request/result
projections and additional classification, corpus, seed and framing operations.

#### Sessions and artifacts

`session.from_initializer` parses the initializer JavaScript and returns:

```json
{
  "transactionToken": "…",
  "alphabet": "…",
  "shuffleKey": [],
  "keys": [0, 0, 0, 0, 0, 0, 0, 0]
}
```

`nonce` is present when the initializer supplies one. These are the parsed
session literals; keep them with the matching capture.

For layout inspection, `pin` takes a traced program and exactly one of
`session` (a document) or `init` (initializer text). It returns the build's
report routes, cipher and header settings. A different build needs a new pin;
decode artifact compilation already performs the pinning it needs.

`artifact.compile` compiles the decode artifact from a traced program (or from
a raw seed with `input_kind:"raw"`, which traces first). `session` and
`site_id` are required. `float_byte_order` is `"little"` (the default) or
`"big"`; set `"big"` when the collecting runtime used big-endian float bytes.
With `input_kind:"raw"`, `name` names the trace (default `"seed.js"`); an
incomplete trace is a `failed` error rather than a partial artifact.

For relaxed source-format compilation, set `"direct":true` explicitly:

- It defaults to false and must be a boolean when present; null, strings and
  numbers return `invalid_request`.
- It allows source-format fallbacks without lifecycle or strict algorithm
  certification. Unsupported constructs can still fail.
- Saved artifacts retain the mode and use it on load. Exact verification still
  requires identical signal and header bytes.

Choose the export for its purpose:

| Document | Purpose |
| --- | --- |
| Decode artifact (`shape-decode-artifact/1`) | Executable recipes, layout, session defaults and float byte order; load it without source files |
| `artifact.compact_schema` (`shape-seed-schema/1`) | Validated descriptive schema for inspection; cannot be loaded as a decoder |

Thread results use a nullable `code` and a `self_check` array of diagnostic
strings. The legacy `trace.program` request field `report_dir` selects the
kernel artifact cache directory.

## Precision

Decode artifacts and decoded payloads can contain JSON integers above 2^53.
They appear as f64 bit patterns in artifact programs (`schema.wire…literal`),
as decoded metadata such as `numberBits`, and as u64 superpack values. The C
ABI passes JSON as text, so it preserves those digits exactly. Your program
must do the same:

- Pass saved artifact files to `shape_generator_load` as raw bytes. Do not
  parse and re-serialize an artifact through a JSON library that stores every
  number as `double`.
- When a request contains a document received from an earlier call, pass its
  text through unchanged, or use a lossless JSON representation. Do not call
  `strtod`/`atof` on integer tokens, and do not convert an artifact or decoded
  payload to `double` before sending it back.
- A JSON library with 64-bit integer or arbitrary-precision support is fine.

If rounding changes one of those digits, `shape_generator_load` may reject the
artifact or a later decode/encode may fail to reproduce the input.

### Choosing a JSON library

| Approach | Works when |
| --- | --- |
| Pass library output text back unchanged | You only extract values with a text scanner, as in the workflow example |
| A JSON library with exact integers (`json-c` with 64-bit support, `yyjson` with `YYJSON_READ_BIGNUM`, a C++ arbitrary-precision type) | You need to build or edit requests and payloads |
| A general-purpose DOM that stores every number as `double` | Never for artifacts, sessions or retained reports |

`json-c` is a common choice and is used by the [signals walkthrough](../signals.md);
keep its 64-bit integer constructors and accessors, and compile with exact
integer support. Whichever library you choose, test one real artifact round
trip before trusting it: load it, decode, re-encode the unchanged payload, and
compare the header values.

## Errors

Dispatch, direct and handle calls return one JSON envelope. Load/compile
functions return a handle or `NULL` and report failures through `error_out`.
`shape_version` returns a plain version string.

```json
{"ok":true,"value":…}
```

```json
{"ok":false,"error":{"code":"invalid_request","message":"missing \"source\""}}
```

Read `ok` first. On success, `value` is the command result. On failure,
`error.code` is stable and `error.message` is diagnostic text. The library
catches Rust panics at the C boundary and returns them as an envelope; it does
not unwind into your process.

| Code | Meaning | Action |
| --- | --- | --- |
| `invalid_request` | Malformed JSON, wrong field/type, invalid UTF-8 or required `NULL` argument | Correct the request and pointer arguments |
| `unknown_command` | Unknown operation or method | Check the name in the relevant reference |
| `failed` | Input understood, but processing failed | Read the diagnostic; check artifact format, matching inputs, trace completion and supported encoding |
| `panic` | An internal panic was caught | Retain a diagnostic for investigation |

Load/compile functions return `NULL` on failure and put the same error envelope
in `*error_out` when supplied. Successful calls clear that slot. Free any prior
slot value yourself before reusing it; free a returned envelope with `shape_free`.
Messages are diagnostic text; match `error.code` for application decisions.

## Threading and lifetime

### Ownership

| Returned value | Release exactly once with |
| --- | --- |
| Result strings from dispatch, direct or handle calls; load/compile error envelopes | `shape_free` |
| `ShapeArtifact` from load/compile | `shape_artifact_free` |
| `ShapeKernel` from load | `shape_kernel_free` |
| Compatibility `ShapeGenerator` from load | `shape_generator_free` |
| Static `shape_version()` string | Never free |

Keep these rules at the call site:

- Keep input buffers readable and unchanged until the call returns. Byte loads
  take a length, need no trailing NUL, and do not retain the input buffer.
- Free library strings with `shape_free`, using the original unmodified pointer.
  Use your allocator's `free` only for your own copies and requests.
- Free each handle once, after its last call. Never use it after release.
  Returned result strings remain valid independently of the handle.
- `shape_free(NULL)` and handle-free functions with `NULL` are no-ops.

A `NULL` or empty request string means `{}`. Command/method strings must be
non-`NULL`, NUL-terminated UTF-8. See [the header](include/shape.h) for pointer
bounds, alignment and `error_out` requirements.

### Threading

Calls block their calling thread and may run concurrently. Immutable artifact,
kernel and generator handles can be shared across calls.

**Wait for all concurrent calls to return before freeing a shared handle.**
Prevent new calls from starting during release. The C API does not add lifetime
locks for you.

Engine work runs on large-stack workers internally; request JSON parsing runs
on the calling thread. Unwinding panics become `panic` error envelopes; process
aborts, invalid pointers, double frees and data races cannot be recovered.

## Performance

- **Calls block.** Each call blocks the calling thread until the engine
  finishes; a trace can take seconds. Run independent calls on different
  threads rather than serializing them behind one.
- **One loaded artifact, many calls.** Loading validates and parses it once.
  Keep the handle; do not reload per request.
- **Decode lazily.** `metadata:false` plus `explain` is the cheap path.
  `verify:true` checks byte-identical re-encoding; enable it when that verification is needed.
- **Engine work uses a large-stack worker internally;** request JSON parsing
  runs on the calling thread.
- **JSON text is copied across the boundary.** Large requests and results cost
  a parse and a copy at each side; keep artifacts in memory as text rather than
  rebuilding them.
- **Reuse loaded kernels.** `shape_kernel_call` reuses parsed kernel facts in
  memory. Named source compilation defaults to no disk cache; an explicit
  `cache` policy opts into directory or environment caching. Legacy
  `shape_call("trace.program", ...)` retains its older `report_dir` behavior.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `undefined symbols: _shape_call` | The library is missing from the link line | Add `-L target/release -lshape`; check the file name for your platform |
| `image not found` / `library not loaded` at run time | The rpath does not reach the library | Use `@loader_path`/`$ORIGIN` as in [Link](#link), or copy the library beside the executable |
| `mis-aligned LINKEDIT string pool` | A stripped Rust dylib was linked | Rebuild with `scripts/build-sdk.sh`; do not use `install_name_tool -id` on it |
| `duplicate symbol` or many undefined Rust symbols | Static link without the Rust runtime's system libraries | Add `-framework CoreFoundation -framework Security` (macOS) or `-ldl -lpthread -lm -lrt` (Linux) |
| `shape_generator_load` returns `NULL` with no message | `error_out` was `NULL` | Pass a `char **` and free the envelope with `shape_free` |
| Crash or corrupt string after a call | A returned string was freed with `free`, or a generator was used after free | Use `shape_free` and free generators after their last call |
| Decode fails on a valid-looking capture | Artifact and capture are from different sites or sessions | Recompile with the capture's `site_id` and matching session |
| Encode fails after an edit | The edit has no supported encoding | Decode the unchanged payload first; edit only named fields; keep the full payload |

## Tests

```sh
cargo test -p shape-c --lib
```

- **ABI unit tests** (the command above) exercise the public entry points,
  input validation, error envelopes and ownership.
- **Consumer checks**: the [C test guide](tests/README.md) covers the C99 and
  C++11 shared and static consumer checks and explicit fixture regeneration.
  It is the right place to start after changing `shape.h` or the build flags.
- **End to end**: compile the [workflow](#workflow) program and run it against
  matching local inputs for recovery and decoding. Use release builds for seed
  recovery; it is expensive.
- **Other bindings**: the Node, Python and Go tests also exercise this ABI, and
  the [shared contract harness](../tests/README.md) compares all five runtimes.
  A change to the ABI's request handling should be checked there, not only in
  the C tests.
