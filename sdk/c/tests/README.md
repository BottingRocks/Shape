# C SDK tests

[`consumer.c`](consumer.c) exercises public ABI ownership, validation and generator
operations. From the repository root, after `scripts/build-sdk.sh`, run all four
C99/C++11 shared/static variants with:

```sh
sdk/c/tests/run-smoke.sh /absolute/path/to/target/release
cargo test --release -p shape-c --lib
```

[`fixtures`](fixtures/README.md) contains the public sample32 inputs: one number, one packed-flags and one constant-string signal, so every binding exercises all three wire shapes.
[`regenerate_fixture.rs`](regenerate_fixture.rs) is an ignored integration test;
normal test runs never mutate snapshots. Compile without running with:

```sh
cargo test --release -p shape-c --test regenerate_fixture --no-run
```

Only regenerate intentionally, using a release build:

```sh
cargo test --release -p shape-c --test regenerate_fixture -- --ignored
```

See the [C guide](../README.md) and [ABI review](../ABI_REVIEW.md).
