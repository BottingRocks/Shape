package shape

import "encoding/json"

// Input kinds accepted by Report, KernelShow and CompileArtifact.
const (
	InputRaw          = "raw"          // an obfuscated seed as captured
	InputDeobfuscated = "deobfuscated" // Deob output (Report, KernelShow)
	InputProgram      = "program"      // TraceProgram output (CompileArtifact)
)

// Documents the library produces and consumes whose structure is large or
// still evolving are passed through as json.RawMessage. Numbers inside typed
// `any` fields decode as json.Number so 64-bit values keep their precision.

// ---- deob / report / kernel / ops ------------------------------------------

// DeobOptions are the options of Deob.
type DeobOptions struct {
	// Artifact also returns the compiled, source-free kernel artifact.
	Artifact bool `json:"artifact,omitempty"`
}

// DeobResult is the result of Deob.
type DeobResult struct {
	Code     string          `json:"code"`
	Artifact json.RawMessage `json:"artifact,omitempty"`
}

// Deob deobfuscates a raw seed.
func Deob(source string, opts DeobOptions) (*DeobResult, error) {
	var out DeobResult
	err := callInto("deob", struct {
		Source string `json:"source"`
		DeobOptions
	}{source, opts}, &out)
	if err != nil {
		return nil, err
	}
	return &out, nil
}

// ReportOptions are the options of Report.
type ReportOptions struct {
	// InputKind is InputRaw (default) or InputDeobfuscated.
	InputKind string `json:"input_kind,omitempty"`
}

// Report compiles the kernel artifact report of a seed.
func Report(source string, opts ReportOptions) (json.RawMessage, error) {
	return Call("report", struct {
		Source string `json:"source"`
		ReportOptions
	}{source, opts})
}

// KernelShowOptions are the options of KernelShow.
type KernelShowOptions struct {
	// InputKind is InputRaw (default) or InputDeobfuscated.
	InputKind string `json:"input_kind,omitempty"`
	// Operations selects operations to include.
	Operations []string `json:"operations,omitempty"`
}

// KernelShow returns the tracer view of a seed's kernel properties.
func KernelShow(source string, opts KernelShowOptions) (json.RawMessage, error) {
	return Call("kernel.show", struct {
		Source string `json:"source"`
		KernelShowOptions
	}{source, opts})
}

// OpsList lists the VM operations of a raw seed.
func OpsList(source string) (json.RawMessage, error) {
	return Call("ops.list", struct {
		Source string `json:"source"`
	}{source})
}

// OpsShow shows one VM operation of a raw seed by its key.
func OpsShow(source string, op uint32) (json.RawMessage, error) {
	return Call("ops.show", struct {
		Source string `json:"source"`
		Op     uint32 `json:"op"`
	}{source, op})
}

// ---- tracing ---------------------------------------------------------------

// Overrides force branch decisions by "pc/state" position.
type Overrides struct {
	// Fake marks branches as fake, with the side always taken.
	Fake map[string]bool `json:"fake,omitempty"`
	// Real marks branches as real code.
	Real []string `json:"real,omitempty"`
}

// TraceProgramOptions are the options of TraceProgram. Zero values mean the
// library defaults.
type TraceProgramOptions struct {
	Name          string     `json:"name,omitempty"`  // seed name, default "seed.js"
	Steps         int        `json:"steps,omitempty"` // step budget, default 1000000
	ThreadCleanup bool       `json:"thread_cleanup,omitempty"`
	NoCleanup     bool       `json:"no_cleanup,omitempty"`
	CheckReuse    bool       `json:"check_reuse,omitempty"`
	Overrides     *Overrides `json:"overrides,omitempty"`
	Jobs          int        `json:"jobs,omitempty"`       // worker threads, default all cores
	ReportDir     string     `json:"report_dir,omitempty"` // kernel artifact cache directory
	Quiet         *bool      `json:"quiet,omitempty"`      // default true
}

// Stop is one function whose trace stopped.
type Stop struct {
	Function string `json:"function"`
	Reason   string `json:"reason"`
}

// Program is the result of TraceProgram.
type Program struct {
	Code          string   `json:"code"`
	Complete      bool     `json:"complete"`
	Traced        int      `json:"traced"`
	Stopped       []Stop   `json:"stopped"`
	KernelGlobals []string `json:"kernel_globals"`
}

// TraceProgram traces every thread of a raw seed and links the program.
func TraceProgram(source string, opts TraceProgramOptions) (*Program, error) {
	var out Program
	err := callInto("trace.program", struct {
		Source string `json:"source"`
		TraceProgramOptions
	}{source, opts}, &out)
	if err != nil {
		return nil, err
	}
	return &out, nil
}

// TraceThreadRequest is the request of TraceThread. Exactly one of Source
// (a raw seed) or Artifact (a kernel artifact document) must be set.
type TraceThreadRequest struct {
	Source       string          `json:"source,omitempty"`
	Artifact     json.RawMessage `json:"artifact,omitempty"`
	PC           int             `json:"pc"`
	State        int             `json:"state"`
	Descriptor   *int            `json:"descriptor,omitempty"`
	InitialSlots json.RawMessage `json:"initial_slots,omitempty"`
	Inputs       json.RawMessage `json:"inputs,omitempty"`
	Steps        int             `json:"steps,omitempty"` // default 1000000
	SelfCheck    bool            `json:"self_check,omitempty"`
	Outline      *bool           `json:"outline,omitempty"` // default true
	ReverseSides bool            `json:"reverse_sides,omitempty"`
	Partial      bool            `json:"partial,omitempty"`
	Overrides    *Overrides      `json:"overrides,omitempty"`
}

// Thread is the result of TraceThread.
type Thread struct {
	// Code is nil when the walk stopped and partial output was not requested.
	Code           *string  `json:"code"`
	Complete       bool     `json:"complete"`
	Stop           string   `json:"stop"`
	Steps          int      `json:"steps"`
	HandlersTraced int      `json:"handlers_traced"`
	Undeclared     []string `json:"undeclared"`
	SelfCheck      []string `json:"self_check"` // diagnostics; empty when none were reported
	SourceHash     string   `json:"source_hash"`
}

// TraceThread walks one VM thread.
func TraceThread(req TraceThreadRequest) (*Thread, error) {
	var out Thread
	if err := callInto("trace.thread", req, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// CleanupStats counts what CleanupThread rewrote.
type CleanupStats struct {
	InlinedBindings  int `json:"inlined_bindings"`
	InlinedSnapshots int `json:"inlined_snapshots"`
	Rounds           int `json:"rounds"`
	InlinedHelpers   int `json:"inlined_helpers"`
	RemovedHelpers   int `json:"removed_helpers"`
	RemainingHelpers int `json:"remaining_helpers"`
	PropertyWrites   int `json:"property_writes"`
	Arrays           int `json:"arrays"`
	Methods          int `json:"methods"`
	Loops            int `json:"loops"`
	FlattenedBlocks  int `json:"flattened_blocks"`
}

// Cleanup is the result of CleanupThread.
type Cleanup struct {
	Code    string       `json:"code"`
	Cleanup CleanupStats `json:"cleanup"`
}

// CleanupThread cleans one emitted thread function.
func CleanupThread(source string) (*Cleanup, error) {
	var out Cleanup
	if err := callInto("cleanup.thread", sourceRequest{source}, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

type sourceRequest struct {
	Source string `json:"source"`
}

// ---- naming ----------------------------------------------------------------

// NamingThreadsOptions are the options of NamingThreads.
type NamingThreadsOptions struct {
	Rename bool            `json:"rename,omitempty"`
	Labels json.RawMessage `json:"labels,omitempty"` // label reference document
}

// NamingCollectorsOptions are the options of NamingCollectors.
type NamingCollectorsOptions struct {
	Rename bool `json:"rename,omitempty"`
}

// Named is the result of NamingThreads and NamingCollectors.
type Named struct {
	Source string          `json:"source"`
	Report json.RawMessage `json:"report"`
}

// NamingThreads names the threads of a traced program.
func NamingThreads(source string, opts NamingThreadsOptions) (*Named, error) {
	var out Named
	err := callInto("naming.threads", struct {
		Source string `json:"source"`
		NamingThreadsOptions
	}{source, opts}, &out)
	if err != nil {
		return nil, err
	}
	return &out, nil
}

// NamingCollectors identifies the signal collectors of a traced program.
func NamingCollectors(source string, opts NamingCollectorsOptions) (*Named, error) {
	var out Named
	err := callInto("naming.collectors", struct {
		Source string `json:"source"`
		NamingCollectorsOptions
	}{source, opts}, &out)
	if err != nil {
		return nil, err
	}
	return &out, nil
}

// NamingPlan plans semantic names for a traced program.
func NamingPlan(source string) (json.RawMessage, error) {
	return Call("naming.plan", sourceRequest{source})
}

// Inventory is the result of NamingInventory.
type Inventory struct {
	FunctionIndex json.RawMessage `json:"function_index"`
	RoleGraph     json.RawMessage `json:"role_graph"`
}

// NamingInventory extracts the function index and role graph of a program.
func NamingInventory(source string) (*Inventory, error) {
	var out Inventory
	if err := callInto("naming.inventory", sourceRequest{source}, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// ---- signals ---------------------------------------------------------------

// SignalsSchemaOptions are the options of SignalsSchema.
type SignalsSchemaOptions struct {
	Name        string `json:"name,omitempty"` // seed name recorded in the report
	LayoutsOnly bool   `json:"layouts_only,omitempty"`
	FinderRoles *bool  `json:"finder_roles,omitempty"` // default true
}

// SignalsSchema analyses the signals of a traced program.
func SignalsSchema(source string, opts SignalsSchemaOptions) (json.RawMessage, error) {
	return Call("signals.schema", struct {
		Source string `json:"source"`
		SignalsSchemaOptions
	}{source, opts})
}

// SignalsShow extracts the signal properties of a traced program.
func SignalsShow(source string) (json.RawMessage, error) {
	return Call("signals.show", sourceRequest{source})
}

// SignalsInputs extracts the program's inputs with context.
func SignalsInputs(source string) (json.RawMessage, error) {
	return Call("signals.inputs", sourceRequest{source})
}

// SignalsContracts extracts the program's signal contracts.
func SignalsContracts(source string) (json.RawMessage, error) {
	return Call("signals.contracts", sourceRequest{source})
}

// PinOptions are the options of Pin: exactly one of Session or Init.
type PinOptions struct {
	Session json.RawMessage `json:"session,omitempty"` // a session document
	Init    string          `json:"init,omitempty"`    // the initializer source
}

// Pin pins the payload layout of a traced program to one session.
func Pin(source string, opts PinOptions) (json.RawMessage, error) {
	return Call("pin", struct {
		Source string `json:"source"`
		PinOptions
	}{source, opts})
}

// SessionFromInitializer extracts the session document from initializer
// source.
func SessionFromInitializer(init string) (json.RawMessage, error) {
	return Call("session.from_initializer", struct {
		Init string `json:"init"`
	}{init})
}

// CompileArtifactOptions are the options of CompileArtifact.
type CompileArtifactOptions struct {
	Direct         bool            `json:"direct,omitempty"`           // source-format mode without lifecycle certification; default false
	Session        json.RawMessage `json:"session"`                    // required session document
	SiteID         string          `json:"site_id"`                    // required
	InputKind      string          `json:"input_kind,omitempty"`       // InputProgram (default) or InputRaw
	Name           string          `json:"name,omitempty"`             // seed name for InputRaw
	FloatByteOrder string          `json:"float_byte_order,omitempty"` // "little" (default) or "big"
}

// CompileArtifact compiles the reusable decode artifact that Load accepts.
func CompileArtifact(source string, opts CompileArtifactOptions) (json.RawMessage, error) {
	return Call("artifact.compile", struct {
		Source string `json:"source"`
		CompileArtifactOptions
	}{source, opts})
}

// CompactSchema returns the compact signal schema of a decode artifact.
func CompactSchema(artifact json.RawMessage) (json.RawMessage, error) {
	return Call("artifact.compact_schema", struct {
		Artifact json.RawMessage `json:"artifact"`
	}{artifact})
}

// ---- generator -------------------------------------------------------------

// Capture is one captured request: site identity plus its signal headers
// (single letters and "a<digits>" chunks).
type Capture struct {
	SiteID  string            `json:"siteId"`
	Headers map[string]string `json:"headers"`
}

// DecodeOptions are the options of Generator.Decode.
type DecodeOptions struct {
	Session  json.RawMessage `json:"session,omitempty"`
	Verify   bool            `json:"verify,omitempty"`
	Metadata bool            `json:"metadata,omitempty"`
}

// ExplainOptions are the options of Generator.Explain.
type ExplainOptions struct {
	Session json.RawMessage `json:"session,omitempty"`
	Verify  bool            `json:"verify,omitempty"`
}

// EncodeRequest is the request of Generator.Encode: exactly one of Decoded
// (a full decoded payload) or Signals (positionalSignals only).
type EncodeRequest struct {
	Decoded *Decoded
	Signals any
	Session json.RawMessage
}

// Signal is one named signal.
type Signal struct {
	// Fields are nil when the signal is not decoded.
	Fields map[string]any `json:"fields"`
	// Status is "decoded", "missing", "unsupported" or "invalid".
	Status   string          `json:"status"`
	Metadata json.RawMessage `json:"metadata"`
}

// DecodeMetadata summarises a decode.
type DecodeMetadata struct {
	OriginalReport json.RawMessage   `json:"originalReport"`
	Signals        map[string]Signal `json:"signals"`
	Decoded        int               `json:"decoded"`
	Missing        int               `json:"missing"`
	ExactReencode  bool              `json:"exactReencode"`
	FloatByteOrder string            `json:"floatByteOrder"`
}

// Decoded is a decoded payload. Explain, Encode and json.Marshal use the current
// fields, including edits to PositionalSignals. Keep Metadata.OriginalReport
// to preserve the original channels, seeds and per-signal prefixes on encode.
type Decoded struct {
	SiteID            string                    `json:"siteId"`
	Errors            []json.RawMessage         `json:"errors"`
	PositionalSignals map[string]map[string]any `json:"positionalSignals"`
	Signals           []json.RawMessage         `json:"signals"`
	KeyedSignals      []json.RawMessage         `json:"keyedSignals"`
	Seeds             [2]uint32                 `json:"seeds"`
	Keysets           [][]string                `json:"keysets"`
	Metadata          DecodeMetadata            `json:"metadata"`

	raw json.RawMessage
}

// UnmarshalJSON keeps the exact document alongside the typed view.
func (d *Decoded) UnmarshalJSON(data []byte) error {
	type view Decoded
	var v view
	if err := decodeValue("decoded", data, &v); err != nil {
		return err
	}
	*d = Decoded(v)
	d.raw = append(json.RawMessage(nil), data...)
	return nil
}

// MarshalJSON serializes the current fields without converting integer values.
func (d Decoded) MarshalJSON() ([]byte, error) {
	type view Decoded
	return json.Marshal(view(d))
}

// JSON returns a copy of the original decoded document, before any edits.
// For the current fields, use json.Marshal instead. It is nil for values built
// directly in Go rather than unmarshalled from JSON.
func (d *Decoded) JSON() json.RawMessage { return append(json.RawMessage(nil), d.raw...) }
