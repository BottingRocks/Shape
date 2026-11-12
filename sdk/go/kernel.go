package shape

/*
#include <stdlib.h>
#include "shape.h"
*/
import "C"

import (
	"encoding/json"
	"runtime"
	"sync"
	"unsafe"
)

// LoadedKernel holds one parsed kernel for repeated tracing without disk cache.
// Do not copy a LoadedKernel; share its pointer. Close waits for active calls.
type LoadedKernel struct {
	mu  sync.RWMutex
	ptr *C.ShapeKernel
}

func LoadKernel(document []byte) (*LoadedKernel, error) {
	var data unsafe.Pointer
	if len(document) == 0 {
		data = C.malloc(1)
	} else {
		data = C.CBytes(document)
	}
	defer C.free(data)
	var failure *C.char
	ptr := C.shape_kernel_load((*C.uint8_t)(data), C.size_t(len(document)), &failure)
	if ptr == nil {
		if failure != nil {
			_, err := takeEnvelope(failure)
			if err != nil {
				return nil, err
			}
		}
		return nil, &Error{Code: CodeFailed, Message: "kernel load failed"}
	}
	if failure != nil {
		C.shape_free(failure)
	}
	k := &LoadedKernel{ptr: ptr}
	runtime.SetFinalizer(k, (*LoadedKernel).Close)
	return k, nil
}
func (k *LoadedKernel) Close() error {
	if k == nil {
		return nil
	}
	k.mu.Lock()
	defer k.mu.Unlock()
	if k.ptr != nil {
		C.shape_kernel_free(k.ptr)
		k.ptr = nil
		runtime.SetFinalizer(k, nil)
	}
	return nil
}
func (k *LoadedKernel) call(operation string, request any) (json.RawMessage, error) {
	if k == nil {
		return nil, &Error{Code: CodeInvalidRequest, Message: "kernel is closed"}
	}
	body, err := requestJSON(request)
	if err != nil {
		return nil, err
	}
	op, req := C.CString(operation), C.CString(body)
	defer C.free(unsafe.Pointer(op))
	defer C.free(unsafe.Pointer(req))
	k.mu.RLock()
	defer k.mu.RUnlock()
	if k.ptr == nil {
		return nil, &Error{Code: CodeInvalidRequest, Message: "kernel is closed"}
	}
	raw, err := takeEnvelope(C.shape_kernel_call(k.ptr, op, req))
	runtime.KeepAlive(k)
	return raw, err
}
func (k *LoadedKernel) JSON() (json.RawMessage, error) {
	raw, err := k.call("kernel.to_json", nil)
	if err != nil {
		return nil, err
	}
	var text string
	if err := decodeValue("kernel.to_json", raw, &text); err != nil {
		return nil, err
	}
	return json.RawMessage(text), nil
}
func (k *LoadedKernel) TraceProgram(source string, options ProgramOptions) (*Program, error) {
	raw, err := k.call("trace.program", struct {
		Source string `json:"source"`
		ProgramOptions
	}{source, options})
	if err != nil {
		return nil, err
	}
	var out Program
	if err := decodeValue("trace.program", raw, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// TraceThread takes thread options only; Source/Artifact in req must be unset.
func (k *LoadedKernel) TraceThread(req TraceThreadRequest) (*Thread, error) {
	if req.Source != "" || len(req.Artifact) != 0 {
		return nil, &Error{Code: CodeInvalidRequest, Message: "loaded kernel thread request must not supply source or artifact"}
	}
	raw, err := k.call("trace.thread", req)
	if err != nil {
		return nil, err
	}
	var out Thread
	if err := decodeValue("trace.thread", raw, &out); err != nil {
		return nil, err
	}
	return &out, nil
}
func (k *LoadedKernel) Reachable(options ReachableOptions) ([]Fingerprint, error) {
	raw, err := k.call("trace.corpus.reachable", options)
	if err != nil {
		return nil, err
	}
	var out []Fingerprint
	if err := decodeValue("trace.corpus.reachable", raw, &out); err != nil {
		return nil, err
	}
	return out, nil
}
