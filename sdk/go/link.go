//go:build !shape_external

package shape

// Default linking: the static library in sdk/go/lib plus the system
// libraries a Rust static library needs. Build with -tags shape_external to
// drop these flags and supply your own through CGO_LDFLAGS (see README.md).

/*
#cgo darwin LDFLAGS: ${SRCDIR}/lib/libshape.a -framework CoreFoundation -framework Security
#cgo linux LDFLAGS: ${SRCDIR}/lib/libshape.a -ldl -lpthread -lm -lrt
*/
import "C"
