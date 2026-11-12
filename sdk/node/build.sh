#!/bin/sh
# Build the Node addon (build/shape.node) against Node's built-in Node-API
# headers and libshape, with plain cc. No node-gyp, no dependencies.
#
#   sdk/node/build.sh
#
# libshape is found in, in order:
#   $SHAPE_LIBRARY_DIR, sdk/node/lib/, <repo>/target/release, <repo>/target/iter,
#   then the existing build/ directory (for rebuilding a packed SDK).
# shape.h is found in, in order:
#   $SHAPE_INCLUDE_DIR, <repo>/sdk/c/include, the library directory, build/
# The library is copied next to shape.node, which loads it via rpath
# (@loader_path on macOS, $ORIGIN on Linux).
# Other knobs: $CC (default cc), $NODE (default node), $NODE_INCLUDE_DIR, $CFLAGS.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
node=${NODE:-node}
cc=${CC:-cc}

case "$(uname -s)" in
    Darwin) libname=libshape.dylib ;;
    *) libname=libshape.so ;;
esac

node_include=${NODE_INCLUDE_DIR:-$("$node" -p 'require("path").join(require("path").dirname(require("path").dirname(process.execPath)), "include", "node")')}
if [ ! -f "$node_include/node_api.h" ]; then
    echo "build.sh: node_api.h not found in $node_include" >&2
    exit 1
fi

libdir=
for dir in "${SHAPE_LIBRARY_DIR:-}" "$here/lib" "$repo/target/release" "$repo/target/iter" "$here/build"; do
    if [ -n "$dir" ] && [ -f "$dir/$libname" ]; then
        libdir=$(cd "$dir" && pwd)
        break
    fi
done
if [ -z "$libdir" ]; then
    echo "build.sh: $libname not found; set SHAPE_LIBRARY_DIR or put it in $here/lib" >&2
    exit 1
fi

incdir=
for dir in "${SHAPE_INCLUDE_DIR:-}" "$repo/sdk/c/include" "$libdir" "$here/build"; do
    if [ -n "$dir" ] && [ -f "$dir/shape.h" ]; then
        incdir=$dir
        break
    fi
done
if [ -z "$incdir" ]; then
    echo "build.sh: shape.h not found; set SHAPE_INCLUDE_DIR" >&2
    exit 1
fi

out="$here/build"
mkdir -p "$out"
if [ "$libdir" != "$out" ]; then
    cp -f "$libdir/$libname" "$out/$libname"
fi
if [ "$(cd "$incdir" && pwd)" != "$out" ]; then
    cp -f "$incdir/shape.h" "$out/shape.h"
fi

case "$(uname -s)" in
    Darwin)
        set -- -shared -fPIC -undefined dynamic_lookup -Wl,-rpath,@loader_path
        ;;
    *)
        # Single quotes keep $ORIGIN literal for the linker.
        set -- -shared -fPIC '-Wl,-rpath,$ORIGIN'
        ;;
esac

# shellcheck disable=SC2086
"$cc" "$@" -O2 -std=c11 -Wall -Wextra ${CFLAGS:-} \
    -I"$node_include" -I"$incdir" \
    "$here/src/addon.c" \
    -L"$out" -lshape \
    -o "$out/shape.node"

echo "built $out/shape.node (libshape from $libdir, shape.h from $incdir)"
