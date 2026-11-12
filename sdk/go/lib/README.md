# sdk/go/lib

The Go package links `libshape.a` from this folder. The library is not
committed: `scripts/build-sdk.sh` builds the release static library and copies
it here after stripping embedded LLVM bitcode (Go 1.27 rejects it). Doing it by
hand requires those sections removed:

    rust-objcopy --remove-section=__LLVM,__bitcode \
      --remove-section=__LLVM,__cmdline in.a sdk/go/lib/libshape.a

See `../README.md` for linking a library from somewhere else (build tag
`shape_external` plus `CGO_LDFLAGS`).
