package shape

/*
#include <stdlib.h>
#include "shape.h"
*/
import "C"

import (
	"encoding/json"
	"runtime"
	"strings"
	"sync"
	"unsafe"
)

// Generator holds one loaded, validated decode artifact for repeated
// decoding. It is safe for concurrent use by many goroutines. Call Close
// when done; a finalizer frees it otherwise.
type Generator struct {
	mu  sync.RWMutex // guards ptr against Close racing with calls
	ptr *C.ShapeGenerator
}

// Load parses and validates a decode artifact (CompileArtifact output or a
// saved artifact file).
func Load(artifact []byte) (*Generator, error) {
	var data *C.uint8_t
	if len(artifact) > 0 {
		data = (*C.uint8_t)(C.CBytes(artifact))
		defer C.free(unsafe.Pointer(data))
	} else {
		// A non-NULL pointer to zero bytes: an empty artifact is invalid JSON.
		data = (*C.uint8_t)(C.malloc(1))
		defer C.free(unsafe.Pointer(data))
	}
	var errOut *C.char
	ptr := C.shape_generator_load(data, C.size_t(len(artifact)), &errOut)
	if ptr == nil {
		if errOut == nil {
			return nil, &Error{Code: CodeFailed, Message: "load failed"}
		}
		_, err := takeEnvelope(errOut)
		if err == nil {
			err = &Error{Code: CodeFailed, Message: "load failed"}
		}
		return nil, err
	}
	if errOut != nil {
		C.shape_free(errOut)
	}
	g := &Generator{ptr: ptr}
	runtime.SetFinalizer(g, (*Generator).Close)
	return g, nil
}

// Close releases the generator. It is idempotent; calls after Close fail
// with CodeInvalidRequest.
func (g *Generator) Close() error {
	if g == nil {
		return nil
	}
	g.mu.Lock()
	defer g.mu.Unlock()
	if g.ptr != nil {
		C.shape_generator_free(g.ptr)
		g.ptr = nil
		runtime.SetFinalizer(g, nil)
	}
	return nil
}

// Call runs a generator method ("decode", "explain", "encode", "schema")
// with request marshalled to JSON, and returns the envelope's value.
func (g *Generator) Call(method string, request any) (json.RawMessage, error) {
	if g == nil {
		return nil, &Error{Code: CodeInvalidRequest, Message: "generator is closed"}
	}
	if strings.ContainsRune(method, '\x00') {
		return nil, &Error{Code: CodeInvalidRequest, Message: "method contains NUL"}
	}
	body, err := requestJSON(request)
	if err != nil {
		return nil, err
	}
	cmethod := C.CString(method)
	defer C.free(unsafe.Pointer(cmethod))
	crequest := C.CString(body)
	defer C.free(unsafe.Pointer(crequest))

	g.mu.RLock()
	defer g.mu.RUnlock()
	if g.ptr == nil {
		return nil, &Error{Code: CodeInvalidRequest, Message: "generator is closed"}
	}
	value, err := takeEnvelope(C.shape_generator_call(g.ptr, cmethod, crequest))
	runtime.KeepAlive(g)
	return value, err
}

// Decode decodes one capture into named signals. capture is a Capture, a
// *Capture, or the capture document as json.RawMessage, []byte or string.
func (g *Generator) Decode(capture any, opts DecodeOptions) (*Decoded, error) {
	value, err := g.Call("decode", struct {
		Capture any `json:"capture"`
		DecodeOptions
	}{document(capture), opts})
	if err != nil {
		return nil, err
	}
	var out Decoded
	if err := decodeValue("decode", value, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// Explain returns one signal's full metadata. decoded is a *Decoded from
// Decode, or the decoded document as json.RawMessage, []byte or string.
func (g *Generator) Explain(decoded any, signal string, opts ExplainOptions) (*Signal, error) {
	value, err := g.Call("explain", struct {
		Decoded any    `json:"decoded"`
		Signal  string `json:"signal"`
		ExplainOptions
	}{document(decoded), signal, opts})
	if err != nil {
		return nil, err
	}
	var out Signal
	if err := decodeValue("explain", value, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// Encode turns named signals back into capture headers.
func (g *Generator) Encode(req EncodeRequest) (*Capture, error) {
	var decoded any
	if req.Decoded != nil {
		decoded = document(req.Decoded)
	}
	value, err := g.Call("encode", struct {
		Decoded any             `json:"decoded,omitempty"`
		Signals any             `json:"signals,omitempty"`
		Session json.RawMessage `json:"session,omitempty"`
	}{decoded, document(req.Signals), req.Session})
	if err != nil {
		return nil, err
	}
	var out Capture
	if err := decodeValue("encode", value, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// Schema returns the compact signal schema of the loaded artifact.
func (g *Generator) Schema() (json.RawMessage, error) {
	return g.Call("schema", nil)
}
