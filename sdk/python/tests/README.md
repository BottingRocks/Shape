# Python SDK tests

Run from the repository root using `uv`. Pure boundary/default/lifecycle checks
need no native library:

```sh
uv run --no-project python -m unittest discover -s sdk/python/tests -k JsonBoundaryTests
uv run --no-project python -m unittest discover -s sdk/python/tests -k GeneratorLifetimeTests
uv run --no-project python -m unittest discover -s sdk/python/tests -k OperationUnitTests
```

For runtime checks use a current contract-v2 optimized native library explicitly.
During coordinated SDK work, read the authoritative path from the release
coordinator's current native-library manifest. Otherwise build via
`scripts/build-sdk.sh` before testing and select the actual release library.
NativeOperationTests fails if SHAPE_LIBRARY
is absent, so new-operation tests cannot silently skip.

```sh
SHAPE_LIBRARY=/absolute/path/to/new/libshape.dylib \
SHAPE_TEST_ARTIFACT="$PWD/sdk/c/tests/fixtures/artifact.json" \
SHAPE_TEST_HEADERS="$PWD/sdk/c/tests/fixtures/capture.json" \
uv run --no-project python -m unittest discover -s sdk/python/tests
```

Use `libshape.so`/`shape.dll` on other platforms. Existing Generator tests require
the two public fixture paths above. No generated captures or private inputs
are needed. `generate_fixtures.py` is an optional regeneration utility, using
an optimized native library and explicit output in ignored `artifacts/`.

- `test_shape.py`: legacy compatibility, strict JSON, exact integer literals,
  errors, concurrency and Generator lifetime/edit round trips.
- `test_operations.py`: explicit policy routing and full cleanup defaults; stale-library
  rejection; typed byte/bit fidelity; direct loaded operations and snapshots;
  kernel reuse, identity errors and disk cache behavior; every portable advanced
  family; public known answers for payload/superpack; README Python syntax and
  all four fixture-supported recipe runtimes.
- `operation_coverage.json`: authoritative operation ID to named public API map
  for cross-binding verification. Loaded methods also cover explicit artifact
  load/export/from_artifact and kernel reuse paths.

Seed fetch tests use a bounded local loopback HTTP server with public synthetic
bytes; they never contact the external network. All other runtime input comes
from checked-in public fixtures. Expensive seed generation, Cargo invocation
and example directories are unnecessary.

Check typing at the supported minimum with:

```sh
uv run --no-project --with 'mypy==1.18.2' mypy --python-version 3.9 sdk/python/shape
```

The wrapper has no runtime dependency on mypy or any third-party package.
