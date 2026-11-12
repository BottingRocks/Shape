#!/usr/bin/env bash
# Build the shape SDK native libraries and the Node addon.
#
# The C ABI crate (sdk/c, package shape-c) is built in release mode.
# The resulting libraries are installed where each binding looks for them:
#
#   sdk/python/shape/lib/libshape.dylib   Python ctypes binding
#   sdk/node/lib/libshape.dylib           Node addon build (sdk/node/build.sh)
#   sdk/go/lib/libshape.a                 Go cgo binding (LLVM bitcode removed)
#
# and finally sdk/node/build.sh compiles the Node addon against the library.
# On macOS the shared library is linked with an @rpath install name and is not
# stripped: a stripped Rust cdylib on this toolchain has an unaligned LINKEDIT
# string table, which the system linker refuses to link against.
# Every output is gitignored; this script never stages or commits anything.
#
# Restartable: rerun it after a failure or after changing the Rust sources.
# Each step overwrites its output. Do not run it at the same time as another
# cargo build (the machine can only take one release link at a time).
#
# Usage: scripts/build-sdk.sh [--help]
set -euo pipefail

usage() {
    cat <<'EOF'
Build the shape SDK native libraries and the Node addon.

Usage:
    scripts/build-sdk.sh [--help]

Steps:
    1. cargo build --release -p shape-c
    2. install libshape.dylib/libshape.so for the Python and Node bindings
    3. copy libshape.a into sdk/go/lib after stripping embedded LLVM bitcode
    4. build the Node addon with sdk/node/build.sh

On macOS the shared library is linked with -C strip=none and an
@rpath/libshape.dylib install name: a stripped Rust cdylib has an unaligned
LINKEDIT string table that the system linker rejects, and the install name lets
the Node addon resolve the copy next to shape.node.

Requires Rust/Cargo, a C compiler, Node with Node-API headers, and rust-objcopy
(rustup component add llvm-tools). This does not build the CLI or install the
Rust crate; Rust consumers depend on the repository root by path.

Environment:
    CARGO         cargo binary (default: cargo)
    CARGO_TARGET_DIR  build output directory (default: <repo>/target)
    NODE          node binary used by sdk/node/build.sh (default: node)
    CC            C compiler used by sdk/node/build.sh (default: cc)
    CFLAGS        extra compiler flags for sdk/node/build.sh
    RUSTUP_HOME   rustup home searched for rust-objcopy (default: ~/.rustup)

The repository is found from this script's own location, so the script works
from any directory. macOS uses libshape.dylib; Linux uses libshape.so. Other
platforms fail.
EOF
}

log() { printf '==> %s\n' "$*"; }
info() { printf '    %s\n' "$*"; }
fail() { printf 'build-sdk.sh: error: %s\n' "$*" >&2; exit 1; }

for arg in "$@"; do
    case $arg in
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'build-sdk.sh: unknown argument: %s\n\n' "$arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

# Follow symlinks so "repo" is the checkout that contains this script, not
# the directory a link happens to live in. A bare file name (script on PATH)
# is resolved first so the repo is still found from another directory.
script_path=${BASH_SOURCE[0]}
if [ "${script_path#*/}" = "$script_path" ]; then
    resolved=$(command -v "$script_path" 2>/dev/null || true)
    [ -z "$resolved" ] || script_path=$resolved
fi
while [ -L "$script_path" ]; do
    script_dir=$(cd -P "$(dirname "$script_path")" && pwd)
    script_path=$(readlink "$script_path")
    case $script_path in
        /*) ;;
        *) script_path=$script_dir/$script_path ;;
    esac
done
repo=$(cd -P "$(dirname "$script_path")/.." && pwd)

case "$(uname -s)" in
    Darwin)
        os=darwin
        shared=libshape.dylib
        section_a=__LLVM,__bitcode
        section_b=__LLVM,__cmdline
        ;;
    Linux)
        os=linux
        shared=libshape.so
        # The equivalent ELF sections (see sdk/go/README.md).
        section_a=.llvmbc
        section_b=.llvmcmd
        ;;
    *)
        fail "unsupported platform: $(uname -s) (only macOS and Linux are supported)"
        ;;
esac

cargo_bin=${CARGO:-cargo}
command -v "$cargo_bin" >/dev/null 2>&1 || fail "cargo not found: $cargo_bin (install Rust or set CARGO)"

# The bindings and this script agree on the workspace target directory. Keep
# an explicitly configured CARGO_TARGET_DIR working; resolve relative paths
# against the repository because that is where cargo is invoked below.
target_dir=${CARGO_TARGET_DIR:-$repo/target}
case $target_dir in
    /*) ;;
    *) target_dir=$repo/$target_dir ;;
esac
if [ -n "${CARGO_BUILD_TARGET:-}" ]; then
    fail "CARGO_BUILD_TARGET is set to '$CARGO_BUILD_TARGET'; this script installs the host build from target/release. Unset it and rerun."
fi

otool=
if [ "$os" = darwin ]; then
    otool=$(command -v otool || true)
    if [ -z "$otool" ] && [ -x /usr/bin/otool ]; then
        otool=/usr/bin/otool
    fi
    [ -n "$otool" ] || fail "otool not found; install the Xcode command line tools"
fi

log "shape SDK build"
info "repository: $repo"
info "platform:   $(uname -s) $(uname -m)"
info "target:     $target_dir/release"

# ---------------------------------------------------------------------------
# 1. Build the C ABI crate. Its [lib] name is "shape", so the artifacts are
#    target/release/libshape.dylib (or .so) and target/release/libshape.a.
#    The release profile uses thin LTO on purpose; do not change it here.
# ---------------------------------------------------------------------------
log "[1/4] Building libshape (release)"
if [ "$os" = darwin ]; then
    # `-C strip=none` keeps the cdylib's LINKEDIT string table aligned for the
    # system linker; the install name makes the dylib relocatable. Both have to
    # be set at link time: install_name_tool would rewrite the load commands
    # and reproduce the alignment problem.
    info "cargo rustc --release -p shape-c -- -C strip=none (-install_name @rpath/libshape.dylib)"
    if ! (cd "$repo" && "$cargo_bin" rustc --release -p shape-c -- \
        -C strip=none \
        -C link-arg=-Wl,-install_name,@rpath/libshape.dylib); then
        fail "cargo rustc failed"
    fi
else
    info "cargo build --release -p shape-c (this can take several minutes)"
    if ! (cd "$repo" && "$cargo_bin" build --release -p shape-c); then
        fail "cargo build failed"
    fi
fi

shared_src=$target_dir/release/$shared
static_src=$target_dir/release/libshape.a
[ -s "$shared_src" ] || fail "built library not found: $shared_src"
[ -s "$static_src" ] || fail "built library not found: $static_src"
if [ "$os" = darwin ]; then
    install_name=$("$otool" -D "$shared_src" | tail -n 1)
    [ "$install_name" = "@rpath/libshape.dylib" ] || fail "unexpected install name '$install_name' in $shared_src"
fi

# ---------------------------------------------------------------------------
# 2. Install the shared library for Python and Node. On macOS the install name
#    must be @rpath/libshape.dylib so the Node addon's @loader_path rpath
#    resolves the copy next to shape.node instead of the build directory.
# ---------------------------------------------------------------------------
log "[2/4] Installing $shared for Python and Node"
for dest_dir in "$repo/sdk/python/shape/lib" "$repo/sdk/node/lib"; do
    mkdir -p "$dest_dir"
    cp -f "$shared_src" "$dest_dir/$shared"
    info "installed $dest_dir/$shared"
done

# ---------------------------------------------------------------------------
# 3. Install the static library for Go. Rust static libraries carry LLVM
#    bitcode in extra sections; recent Go toolchains reject a library that
#    still has it, so strip it while copying. Keep a previous good copy if
#    anything fails: the new file is written to a temporary name first.
# ---------------------------------------------------------------------------

# Prefer rust-objcopy on PATH, then the one belonging to the active rustc
# sysroot, then any rustup toolchain's copy.
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}
find_rust_objcopy() {
    local candidate sysroot
    if candidate=$(command -v rust-objcopy 2>/dev/null); then
        printf '%s\n' "$candidate"
        return 0
    fi
    if command -v rustc >/dev/null 2>&1; then
        sysroot=$(rustc --print sysroot 2>/dev/null || true)
        if [ -n "$sysroot" ]; then
            for candidate in "$sysroot"/lib/rustlib/*/bin/rust-objcopy; do
                if [ -x "$candidate" ]; then
                    printf '%s\n' "$candidate"
                    return 0
                fi
            done
        fi
    fi
    for candidate in "$rustup_home"/toolchains/*/lib/rustlib/*/bin/rust-objcopy; do
        if [ -x "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

# Check the sections in the *temporary* library before replacing the installed
# one. Skip (with a note) when the inspecting tool is unavailable.
verify_no_bitcode() {
    local lib=$1 listing
    case $os in
        darwin)
            if ! command -v otool >/dev/null 2>&1; then
                info "otool not found; skipping the bitcode section check"
                return 0
            fi
            if ! listing=$(otool -l "$lib" 2>/dev/null); then
                info "otool could not read $lib; skipping the bitcode section check"
                return 0
            fi
            case $listing in
                *'sectname __bitcode'* | *'sectname __cmdline'*)
                    fail "LLVM bitcode sections remain in $lib"
                    ;;
            esac
            ;;
        linux)
            if ! command -v readelf >/dev/null 2>&1; then
                info "readelf not found; skipping the bitcode section check"
                return 0
            fi
            if ! listing=$(readelf -S "$lib" 2>/dev/null); then
                info "readelf could not read $lib; skipping the bitcode section check"
                return 0
            fi
            case $listing in
                *'.llvmbc'* | *'.llvmcmd'*)
                    fail "LLVM bitcode sections remain in $lib"
                    ;;
            esac
            ;;
    esac
}

log "[3/4] Installing libshape.a for Go (stripping LLVM bitcode)"
rust_objcopy=$(find_rust_objcopy || true)
if [ -z "$rust_objcopy" ]; then
    fail "rust-objcopy not found (looked on PATH, in the rustc sysroot, and under $rustup_home/toolchains/*/lib/rustlib/*/bin/). Install it with: rustup component add llvm-tools"
fi
info "rust-objcopy: $rust_objcopy"

go_lib=$repo/sdk/go/lib
mkdir -p "$go_lib"
obj_tmp=$go_lib/.libshape.a.tmp.$$
trap 'rm -f "$obj_tmp"' EXIT
if ! "$rust_objcopy" \
    --remove-section="$section_a" \
    --remove-section="$section_b" \
    "$static_src" "$obj_tmp"; then
    fail "rust-objcopy failed to strip LLVM bitcode from $static_src"
fi
[ -s "$obj_tmp" ] || fail "rust-objcopy produced no output"
verify_no_bitcode "$obj_tmp"
mv -f "$obj_tmp" "$go_lib/libshape.a"
trap - EXIT
info "installed $go_lib/libshape.a (LLVM bitcode stripped)"

# ---------------------------------------------------------------------------
# 4. Build the Node addon against the freshly installed library.
#    sdk/node/build.sh honours NODE, CC and CFLAGS from the environment.
# ---------------------------------------------------------------------------
log "[4/4] Building the Node addon"
[ -x "$repo/sdk/node/build.sh" ] || fail "sdk/node/build.sh is missing or not executable"
info "NODE=${NODE:-node} CC=${CC:-cc}"
if ! (cd "$repo/sdk/node" && ./build.sh); then
    fail "sdk/node/build.sh failed"
fi
[ -f "$repo/sdk/node/build/shape.node" ] || fail "sdk/node/build.sh did not produce sdk/node/build/shape.node"

log "SDK build complete"
info "Python: sdk/python/shape/lib/$shared"
info "Node:   sdk/node/lib/$shared"
info "Go:     sdk/go/lib/libshape.a"
info "Addon:  sdk/node/build/shape.node"
