# Capture acquisition

This folder implements `shape seeds` in the root binary. `fetch` is the only
network command; `sites`, `parse`, and `import` work offline. It
stores launchers and raw kernels as data for later analysis and never executes
their JavaScript or WebAssembly.

From the repository root, list configured entry points offline:

```sh
shape seeds sites
```

Use a validated registry name in `shape seeds fetch --site NAME --count 1`.
This live request writes to the default Git-ignored `seeds/` directory.
[Capture acquisition](../../docs/deob/seeds.md) covers URLs, limits, and output naming.

## Files

| File | Responsibility |
| --- | --- |
| [`sites.rs`](sites.rs) | Registry names and per-site request details |
| [`http.rs`](http.rs) | Bounded HTTP, redirects, headers, and origin handling |
| [`collect.rs`](collect.rs) | Launcher/kernel acquisition flow |
| [`script.rs`](script.rs) | Oxc classification of saved launchers and kernels |
| [`seed.rs`](seed.rs) | Capture token validation and kernel URL resolution |
| [`store.rs`](store.rs) | Exact-byte storage, duplicates, and digest-suffixed collisions |
| [`error.rs`](error.rs), [`mod.rs`](mod.rs) | Typed errors and exports |

## Library flow and extension points

`script::classify` accepts saved bytes and returns a launcher or kernel
classification. `parse_launcher` and `parse_kernel` are useful when the caller
already knows which role is expected. Classification does not fetch a URL or
establish that the kernel's handlers can be traced.

For live acquisition, `collect::fetch_seed` takes an `HttpClient`, launcher URL,
and `FetchOptions`, and returns a `FetchedSeed` without writing files.
`fetch_direct_kernel` handles the direct-kernel protocol. Collection composes
those paths with `SeedStore`. The injectable HTTP seam lets tests exercise
redirects, headers, bad responses, and duplicates without contacting a site.

When adding a site, put its entry point and protocol in `sites.rs`. If the
launcher syntax is new, extend the structural parser in `script.rs` with a
positive example and a similar unsupported form. Transport limits and redirect
behavior belong in `http.rs`; byte identity and naming belong in `store.rs`.

## Boundaries

Launcher parsing proves a supported script-load shape; it is not a browser or
a promise that every fetched script is a valid kernel. A direct-kernel URL can
yield script bytes or supported inline HTML; one same-origin refresh hop is
bounded. Relative kernel URLs need the correct document base, which a
standalone fetch may not know.

The store never overwrites a different raw kernel for the same capture token.
Identical bytes are duplicates; changed bytes use a SHA-256-suffixed variant.
Token validation prevents path escape. Captured files remain outside
`src/`, and their retention is the user's choice.

Run the offline acquisition checks from the repository root:

```sh
cargo test --test seeds_acquisition --test seeds_adversarial --test seeds_cli
```
