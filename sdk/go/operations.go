package shape

/*
#include <stdlib.h>
#include "shape.h"
*/
import "C"

import (
	"encoding/json"
	"strings"
	"unsafe"
)

// CallOperation invokes the operation-oriented v2 contract. Call retains the
// legacy dispatcher and its defaults. Prefer typed operations where available.
func CallOperation(operation string, request any) (json.RawMessage, error) {
	if strings.ContainsRune(operation, '\x00') {
		return nil, &Error{Code: CodeInvalidRequest, Message: "operation contains NUL"}
	}
	body, err := requestJSON(request)
	if err != nil {
		return nil, err
	}
	op := C.CString(operation)
	defer C.free(unsafe.Pointer(op))
	req := C.CString(body)
	defer C.free(unsafe.Pointer(req))
	return takeEnvelope(C.shape_call_v2(op, req))
}

func operationInto[T any](operation string, request any) (T, error) {
	var out T
	raw, err := CallOperation(operation, request)
	if err != nil {
		return out, err
	}
	err = decodeValue(operation, raw, &out)
	return out, err
}

// VersionInfo describes the native operation contract and supported operations.
type VersionInfo struct {
	Version         string   `json:"version"`
	Commands        []string `json:"commands"`
	ContractVersion int      `json:"contract_version"`
}

func NativeVersion() (VersionInfo, error) { return operationInto[VersionInfo]("version", nil) }
func Commands() ([]string, error)         { v, err := NativeVersion(); return v.Commands, err }

// CacheMode selects disk kernel compilation reuse, independently of loaded handles.
type CacheMode string

const (
	CacheDisabled    CacheMode = "disabled"
	CacheDirectory   CacheMode = "directory"
	CacheEnvironment CacheMode = "environment"
)

// CachePolicy is per call. The zero value disables all disk cache I/O.
// Environment opts into the native legacy environment policy; it never sets it.
type CachePolicy struct {
	Mode      CacheMode `json:"mode"`
	Directory string    `json:"directory,omitempty"`
}

func (p CachePolicy) MarshalJSON() ([]byte, error) {
	type wire CachePolicy
	if p.Mode == "" {
		p.Mode = CacheDisabled
	}
	return json.Marshal(wire(p))
}

// ProgramOptions uses the Rust defaults: parallel thread cleanup and full
// mechanical finishing. Nil ThreadCleanup/Quiet mean true; nil NoCleanup means
// false; nil Jobs uses available cores. Explicit NoCleanup=true keeps diagnostic
// output that skips the full pipeline. Semantic naming is optional.
type ProgramOptions struct {
	Name          string      `json:"name,omitempty"`
	Cache         CachePolicy `json:"cache"`
	Steps         int         `json:"steps,omitempty"`
	ThreadCleanup *bool       `json:"thread_cleanup,omitempty"`
	NoCleanup     *bool       `json:"no_cleanup,omitempty"`
	CheckReuse    bool        `json:"check_reuse,omitempty"`
	Overrides     *Overrides  `json:"overrides,omitempty"`
	Jobs          *int        `json:"jobs,omitempty"`
	Quiet         *bool       `json:"quiet,omitempty"`
}

func TraceProgramWithOptions(source string, options ProgramOptions) (*Program, error) {
	return operationInto[*Program]("trace.program", struct {
		Source string `json:"source"`
		ProgramOptions
	}{source, options})
}

// TraceProgramWithArtifact reuses an explicit kernel document. Cache is unused.
// For repeated kernel operations use LoadKernel and its methods instead.
func TraceProgramWithArtifact(source string, artifact json.RawMessage, options ProgramOptions) (*Program, error) {
	return operationInto[*Program]("trace.program", struct {
		Source   string          `json:"source"`
		Artifact json.RawMessage `json:"artifact"`
		ProgramOptions
	}{source, artifact, options})
}

// ArtifactOptions compiles traced program text by default. InputRaw selects
// raw source and the full cleanup policy. No output is saved implicitly.
type ArtifactOptions struct {
	Session        json.RawMessage `json:"session"`
	SiteID         string          `json:"site_id"`
	InputKind      string          `json:"input_kind,omitempty"`
	Name           string          `json:"name,omitempty"`
	Cache          CachePolicy     `json:"cache"`
	Direct         bool            `json:"direct,omitempty"`
	FloatByteOrder string          `json:"float_byte_order,omitempty"`
}

// CompileArtifactWithOptions explicitly returns a document for caller persistence.
func CompileArtifactWithOptions(source string, options ArtifactOptions) (json.RawMessage, error) {
	return CallOperation("artifact.compile", artifactCompileRequest{source, options})
}

type artifactCompileRequest struct {
	Source string `json:"source"`
	ArtifactOptions
}

func SignalsCatalog() (json.RawMessage, error) { return CallOperation("signals.catalog", nil) }
func SignalsDescribe(name string) (json.RawMessage, error) {
	return CallOperation("signals.describe", struct {
		Name string `json:"name"`
	}{name})
}

type SourceSchemaOptions struct {
	Direct bool `json:"direct,omitempty"`
}

func SignalsCompactSchema(source string, options SourceSchemaOptions) (json.RawMessage, error) {
	return CallOperation("signals.compact_schema", struct {
		Source string `json:"source"`
		SourceSchemaOptions
	}{source, options})
}

// KernelDocument validates/canonicalizes an explicit kernel artifact document.
func KernelDocument(artifact json.RawMessage) (json.RawMessage, error) {
	return CallOperation("kernel.load", struct {
		Artifact json.RawMessage `json:"artifact"`
	}{artifact})
}
func KernelJSON(artifact json.RawMessage) (string, error) {
	return operationInto[string]("kernel.to_json", struct {
		Artifact json.RawMessage `json:"artifact"`
	}{artifact})
}
