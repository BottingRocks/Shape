// Package shape exposes operation-oriented source analysis, tracing, naming,
// signal inspection and decode/encode operations. Artifact.Load and
// Artifact.Compile produce immutable validated LoadedArtifact handles for
// repeated Decode, Explain, Encode and Schema calls. Close handles when done.
//
// Trace.Program uses explicit per-call cache policy (disabled by default) and
// fast per-thread cleanup without whole-program finishing. Loaded memory reuse
// is independent of disk caching. Seed acquisition and persistence are explicit.
// Generator, Load, Call and the older trace/compile option types remain available
// for compatibility.
//
// It wraps the C ABI in sdk/c/include/shape.h. Every call sends one JSON
// request and receives one JSON envelope; failures come back as *Error.
// All functions are safe to call from many goroutines at once. Calls block
// the calling goroutine (and its OS thread) and some, such as TraceProgram,
// run for seconds.
//
// JSON integers are exact. Requests are marshalled with encoding/json (int64,
// uint64, json.Number and json.RawMessage keep every digit), and values are
// decoded with json.Decoder.UseNumber, so numbers in any-typed fields are
// json.Number rather than float64. Decode artifacts and payloads therefore
// keep 64-bit values above 2^53 exact end to end.
package shape

/*
#cgo CFLAGS: -I${SRCDIR}/../c/include
#include <stdlib.h>
#include <string.h>
#include "shape.h"
*/
import "C"

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
	"unsafe"
)

// Version returns the SDK version, e.g. "0.1.0".
func Version() string {
	return C.GoString(C.shape_version())
}

// Call runs one stateless command (for example "deob", "trace.program" or
// "signals.schema") with request marshalled to JSON (nil means {}), and
// returns the envelope's value exactly as the library wrote it: 64-bit
// integers keep their decimal digits. The typed helpers in this package wrap
// Call.
func Call(command string, request any) (json.RawMessage, error) {
	if strings.ContainsRune(command, '\x00') {
		return nil, &Error{Code: CodeInvalidRequest, Message: "command contains NUL"}
	}
	body, err := requestJSON(request)
	if err != nil {
		return nil, err
	}
	ccommand := C.CString(command)
	defer C.free(unsafe.Pointer(ccommand))
	crequest := C.CString(body)
	defer C.free(unsafe.Pointer(crequest))
	return takeEnvelope(C.shape_call(ccommand, crequest))
}

// callInto runs a command and unmarshals its value into out.
func callInto(command string, request any, out any) error {
	value, err := Call(command, request)
	if err != nil {
		return err
	}
	return decodeValue(command, value, out)
}

// requestJSON renders a request: nil → "{}", json.RawMessage / []byte /
// string pass through as JSON text, anything else is marshalled.
func requestJSON(request any) (string, error) {
	body, err := marshalRequest(request)
	if err == nil && strings.ContainsRune(body, '\x00') {
		return "", &Error{Code: CodeInvalidRequest, Message: "request contains NUL"}
	}
	return body, err
}

func marshalRequest(request any) (string, error) {
	switch r := request.(type) {
	case nil:
		return "{}", nil
	case json.RawMessage:
		return string(r), nil
	case []byte:
		return string(r), nil
	case string:
		return r, nil
	}
	body, err := json.Marshal(request)
	if err != nil {
		return "", &Error{Code: CodeInvalidRequest, Message: "request: " + err.Error()}
	}
	return string(body), nil
}

// takeEnvelope copies and frees a returned C string, then unwraps it.
func takeEnvelope(text *C.char) (json.RawMessage, error) {
	if text == nil {
		return nil, &Error{Code: CodeFailed, Message: "shape returned NULL"}
	}
	defer C.shape_free(text)
	return unwrap(C.GoBytes(unsafe.Pointer(text), C.int(C.strlen(text))))
}

type envelope struct {
	OK    bool            `json:"ok"`
	Value json.RawMessage `json:"value"`
	Error *Error          `json:"error"`
}

func unwrap(text []byte) (json.RawMessage, error) {
	var env envelope
	if err := json.Unmarshal(text, &env); err != nil {
		return nil, &Error{Code: CodeFailed, Message: "malformed envelope: " + err.Error()}
	}
	if !env.OK {
		if env.Error == nil {
			return nil, &Error{Code: CodeFailed, Message: "error envelope without error"}
		}
		return nil, env.Error
	}
	return env.Value, nil
}

// decodeValue decodes a result value. UseNumber keeps every number as a
// json.Number (the original digits) in any-typed destinations, so 64-bit
// values never pass through float64.
func decodeValue(what string, value json.RawMessage, out any) error {
	dec := json.NewDecoder(bytes.NewReader(value))
	dec.UseNumber()
	if err := dec.Decode(out); err != nil {
		return &Error{Code: CodeFailed, Message: fmt.Sprintf("%s result: %v", what, err)}
	}
	return nil
}

// document turns a JSON document argument into a request field: raw bytes
// and strings are sent as JSON text (the library parses string documents),
// anything else is marshalled as is.
func document(v any) any {
	switch d := v.(type) {
	case []byte:
		return json.RawMessage(d)
	case *Decoded:
		if d == nil {
			return nil
		}
		return d
	}
	return v
}
