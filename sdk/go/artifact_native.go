package shape

/*
#include <stdlib.h>
#include "shape.h"
*/
import "C"

import (
	"runtime"
	"unsafe"
)

// CompileLoadedArtifact compiles directly into one validated native handle.
// It does not export/reload JSON or save files. Use JSON for deliberate export,
// or CompileArtifactWithOptions when only a serialized document is required.
func CompileLoadedArtifact(source string, options ArtifactOptions) (*LoadedArtifact, error) {
	body, err := requestJSON(artifactCompileRequest{source, options})
	if err != nil {
		return nil, err
	}
	request := C.CString(body)
	defer C.free(unsafe.Pointer(request))
	var failure *C.char
	ptr := C.shape_artifact_compile(request, &failure)
	if ptr == nil {
		if failure != nil {
			_, err := takeEnvelope(failure)
			if err != nil {
				return nil, err
			}
		}
		return nil, &Error{Code: CodeFailed, Message: "artifact compile failed"}
	}
	if failure != nil {
		C.shape_free(failure)
	}
	g := &Generator{ptr: ptr}
	runtime.SetFinalizer(g, (*Generator).Close)
	return &LoadedArtifact{generator: g}, nil
}
