# Validation record

The updated harness passed on macOS arm64 against the optimized native provider
and the separate Node, Python and Go worker checkouts. The native provider was
`11c21b397ddb1eb89a8aa38c5559e0c99bbafc7d`, including the compile-to-loaded
producer. No Cargo build or large seed recovery ran in this worker.

Invocation from this checkout, with operator-supplied paths for the native
optimized build and each worker checkout (the absolute values are retained in
the shared coordination log):

```sh
uv run --no-project python sdk/tests/contract.py \
  --release-dir "$NATIVE_RELEASE_DIR" \
  --provider-root "$PROVIDER_ROOT" \
  --node-root "$NODE_ROOT" \
  --python-root "$PYTHON_ROOT" \
  --go-root "$GO_ROOT" \
  --output /tmp/shape-contract-worker-verified
```

The optimized directory was supplied read-only; drivers and the copied Node
addon were built in the output directory. All inputs were public committed
fixtures. The pinned native library SHA-256 was
`9ba97d516a03226a42e71e72c605422c715ff5e125696ab84fa0d2970c9536e3`.
The invocation exited 0:

```text
TYPES strict ESM value/type imports and 67 public operation paths
RESULT 112 cases, 561 runtime checks, 4 explicit skips, 0 failures
```

The four skips are the existing individual Python top-level raw malformed/
nonobject request checks. Their nested-document counterparts ran in all five
runtimes. Neither a language nor a result field was omitted. Both legacy generic
interfaces and new public named operations ran. Direct signals-only encoding
by every runtime was decoded by all five. The tiny public VM fixture exercised
fast defaults, all cache policies and loaded kernel reuse; it is not evidence
for arbitrary real captured-seed recovery.

Earlier attempts exposed and corrected harness issues: a pending external
compile producer initially blocked Go compilation; Node adapter positional
fields were duplicated in options; isolated TypeScript resolution needed an
explicit Node type root; seed acquisition correctly rejected an undersized
invented test snippet; Environment caches use hashed filenames; Go host bytes
needed u8-array test transport; and superpack canonically orders object tuples.
The final pass includes those corrections without filtering result fields or
loosening numeric comparison. The shared coordination log retains the failed
attempts and partial-run scope.

The complete Python program in [signals.md](../signals.md) also ran successfully
in an isolated temporary directory against the same pinned native library and
public sample32 fixture. It checked unchanged encoding, explain, field edits,
verified decode and fresh signals-only encoding. All 44 relative document
links in the owned guides resolve, with the provider-owned registry resolved
from its worktree pending integration. Python/Node syntax, strict ESM type
coverage, and diff whitespace checks passed.

The language worker sources were read directly; their final integration still
needs to match the reviewed trees. Rerun the [one-command suite](README.md)
when integrating later source changes or a rebuilt provider. No baseline
binary check is being presented as new API proof. Linux is supported by the
harness but was not executed in this session.
