#!/usr/bin/env bash
# After building shape-c, run: sdk/c/tests/run-smoke.sh /absolute/target/release
# Runs real C99 and C++11 consumers against both distributions. All outputs are
# temporary; the supplied native libraries are never modified.
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
lib=$(cd "${1:?usage: run-smoke.sh /path/to/target/release}" && pwd)
out=$(mktemp -d "${TMPDIR:-/tmp}/shape-c-consumer.XXXXXX")
trap 'rm -rf "$out"' EXIT
case $(uname -s) in
    Darwin)
        shared=libshape.dylib
        rpath='@loader_path'
        system=(-framework CoreFoundation -framework Security)
        ;;
    Linux)
        shared=libshape.so
        rpath='$ORIGIN'
        system=(-ldl -lpthread -lm -lrt)
        ;;
    *) echo 'This smoke check supports macOS and Linux' >&2; exit 1 ;;
esac
cp "$lib/$shared" "$out/$shared"
for language in c c++; do
    if [ "$language" = c ]; then
        compiler=${CC:-cc}
        standard=c99
    else
        compiler=${CXX:-c++}
        standard=c++11
    fi
    for mode in dynamic static; do
        if [ "$mode" = dynamic ]; then
            link=(-L "$out" -lshape "-Wl,-rpath,$rpath")
        else
            link=("$lib/libshape.a" "${system[@]}")
        fi
        "$compiler" -x "$language" -std="$standard" -Wall -Wextra -Werror -pedantic \
            -I "$root/sdk/c/include" -c "$root/sdk/c/tests/consumer.c" -o "$out/consumer.o"
        "$compiler" "$out/consumer.o" "${link[@]}" -o "$out/consumer"
        echo "$language $mode"
        "$out/consumer" "$root/sdk/c/tests/fixtures/artifact.json"
    done
done
