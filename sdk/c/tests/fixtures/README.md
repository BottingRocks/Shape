# Public sample32 fixtures

[`artifact.json`](artifact.json) and [`capture.json`](capture.json) are a matching
synthetic decode artifact and capture for site `example`. They decode three
signals covering the three wire shapes: `DeviceMemory.deviceMemory == 32`
(number), `StorageAccessApiPresence` (packed booleans), and
`ConstantEmptyString.constant == ""` (string), retaining exact 64-bit literals.
No private capture is needed.

The [ignored regeneration utility](../README.md) uses the public wire references
and source/session fixtures. Ordinary tests read these snapshots without changing
them. To run Node, Python and Go generator suites from the repository root after
`scripts/build-sdk.sh`:

```sh
export SHAPE_TEST_ARTIFACT="$PWD/sdk/c/tests/fixtures/artifact.json"
export SHAPE_TEST_HEADERS="$PWD/sdk/c/tests/fixtures/capture.json"
npm --prefix sdk/node test
uv run --no-project python -m unittest discover sdk/python/tests
(cd sdk/go && go test -race ./...)
```

See the [shared contract harness](../../../tests/README.md) for all five runtimes.
