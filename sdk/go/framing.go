package shape

import (
	"encoding/json"
	"fmt"
)

// CipherProfile uses this format's counter/seed geometry, not an IETF nonce.
type CipherProfile struct {
	IVs          [4]uint32 `json:"ivs"`
	Counter      uint64    `json:"counter"`
	DoubleRounds uint32    `json:"double_rounds"`
}
type PayloadProfile struct {
	Format   string        `json:"format"`
	Cipher   CipherProfile `json:"cipher"`
	Alphabet string        `json:"alphabet"`
	Keys     [8]uint32     `json:"keys"`
}
type HeaderContext struct {
	B bool    `json:"b"`
	F bool    `json:"f"`
	C *string `json:"c,omitempty"`
	D *string `json:"d,omitempty"`
	Z *string `json:"z,omitempty"`
}
type HeaderProfile struct {
	PayloadProfile
	HeaderName       string         `json:"header_name"`
	TransactionToken *string        `json:"transaction_token,omitempty"`
	ChunkSize        *uint64        `json:"chunk_size,omitempty"`
	HeaderContext    *HeaderContext `json:"header_context,omitempty"`
	PositionalCount  *uint64        `json:"positional_count,omitempty"`
	Keysets          *[][]string    `json:"keysets,omitempty"`
}

// Headers contains full header names, unlike Capture.Headers' suffix keys.
type Headers map[string]string

// PayloadData selects raw bytes or a versioned container document. Bytes are
// opaque; container entries are not interpreted as named signal fields.
type PayloadData struct {
	Kind      string          `json:"kind"`
	Bytes     Bytes           `json:"-"`
	Container json.RawMessage `json:"-"`
}

func (d PayloadData) MarshalJSON() ([]byte, error) {
	if d.Kind == "raw" {
		return json.Marshal(struct {
			Kind  string `json:"kind"`
			Bytes Bytes  `json:"bytes"`
		}{d.Kind, d.Bytes})
	}
	if d.Kind == "container" {
		return json.Marshal(struct {
			Kind      string          `json:"kind"`
			Container json.RawMessage `json:"container"`
		}{d.Kind, d.Container})
	}
	return nil, &Error{Code: CodeInvalidRequest, Message: "payload data kind must be raw or container"}
}
func (d *PayloadData) UnmarshalJSON(data []byte) error {
	var wire struct {
		Kind      string          `json:"kind"`
		Bytes     Bytes           `json:"bytes"`
		Container json.RawMessage `json:"container"`
	}
	if err := json.Unmarshal(data, &wire); err != nil {
		return err
	}
	if wire.Kind != "raw" && wire.Kind != "container" {
		return &Error{Code: CodeFailed, Message: "unknown payload data kind"}
	}
	*d = PayloadData{wire.Kind, wire.Bytes, wire.Container}
	return nil
}

type Payload struct {
	Seeds         [2]uint32      `json:"seeds"`
	Data          PayloadData    `json:"data"`
	HeaderContext *HeaderContext `json:"header_context,omitempty"`
}

// ReportEntry is the source pair of generic values and opaque extra bytes.
// A nil *ReportEntry in a positional slot is the missing-entry sentinel.
type ReportEntry struct {
	Values []PackValue
	Extra  Bytes
}

func (e ReportEntry) MarshalJSON() ([]byte, error) {
	values := e.Values
	if values == nil {
		values = []PackValue{}
	}
	return json.Marshal([2]any{values, e.Extra})
}
func (e *ReportEntry) UnmarshalJSON(data []byte) error {
	var pair []json.RawMessage
	if err := json.Unmarshal(data, &pair); err != nil {
		return err
	}
	if len(pair) != 2 {
		return fmt.Errorf("report entry needs values and extra bytes")
	}
	var result ReportEntry
	if err := json.Unmarshal(pair[0], &result.Values); err != nil {
		return err
	}
	if err := json.Unmarshal(pair[1], &result.Extra); err != nil {
		return err
	}
	*e = result
	return nil
}

type KeyedReportEntry struct {
	Name  uint32
	Entry *ReportEntry
}

func (e KeyedReportEntry) MarshalJSON() ([]byte, error) { return json.Marshal([2]any{e.Name, e.Entry}) }
func (e *KeyedReportEntry) UnmarshalJSON(data []byte) error {
	var pair []json.RawMessage
	if err := json.Unmarshal(data, &pair); err != nil {
		return err
	}
	if len(pair) != 2 {
		return fmt.Errorf("keyed entry needs name and report entry")
	}
	var result KeyedReportEntry
	if err := json.Unmarshal(pair[0], &result.Name); err != nil {
		return err
	}
	if err := json.Unmarshal(pair[1], &result.Entry); err != nil {
		return err
	}
	*e = result
	return nil
}

// FramedReport is the generic packed report, distinct from named Decoded values.
// Typed entries preserve tuple order and exact PackValue number bits.
type FramedReport struct {
	Errors            []PackValue        `json:"errors"`
	PositionalSignals []*ReportEntry     `json:"positionalSignals"`
	Signals           []PackValue        `json:"signals"`
	KeyedSignals      []KeyedReportEntry `json:"keyedSignals"`
	Seeds             [2]uint32          `json:"seeds"`
	Keysets           [][]string         `json:"keysets"`
	HeaderContext     *HeaderContext     `json:"header_context,omitempty"`
}

func (r FramedReport) MarshalJSON() ([]byte, error) {
	type wire FramedReport
	if r.Errors == nil {
		r.Errors = []PackValue{}
	}
	if r.PositionalSignals == nil {
		r.PositionalSignals = []*ReportEntry{}
	}
	if r.Signals == nil {
		r.Signals = []PackValue{}
	}
	if r.KeyedSignals == nil {
		r.KeyedSignals = []KeyedReportEntry{}
	}
	if r.Keysets == nil {
		r.Keysets = [][]string{}
	}
	return json.Marshal(wire(r))
}

type FinalizedHeaders struct {
	Text    string        `json:"text"`
	Context HeaderContext `json:"context"`
}
type ReportGeometry struct {
	PositionalCount uint64     `json:"positional_count"`
	Keysets         [][]string `json:"keysets,omitempty"`
}

// HeadersCodec and PayloadCodec validate profiles. Portable calls explicitly
// pass the returned profile; they do not create persistent native codec handles.
func HeadersCodec(profile HeaderProfile) (*HeaderProfile, error) {
	return operationInto[*HeaderProfile]("headers.codec", struct {
		Profile HeaderProfile `json:"profile"`
	}{profile})
}
func PayloadCodec(profile PayloadProfile) (*PayloadProfile, error) {
	return operationInto[*PayloadProfile]("payload.codec", struct {
		Profile PayloadProfile `json:"profile"`
	}{profile})
}
func HeadersParse(text string) (Headers, error) {
	return operationInto[Headers]("headers.parse", struct {
		Text string `json:"text"`
	}{text})
}
func HeadersEncode(profile HeaderProfile, report FramedReport) (Headers, error) {
	return operationInto[Headers]("headers.encode", struct {
		Profile HeaderProfile `json:"profile"`
		Report  FramedReport  `json:"report"`
	}{profile, report})
}
func HeadersDecode(profile HeaderProfile, headers Headers, geometry ReportGeometry) (*FramedReport, error) {
	return operationInto[*FramedReport]("headers.decode", struct {
		Profile HeaderProfile `json:"profile"`
		Headers Headers       `json:"headers"`
		ReportGeometry
	}{profile, headers, geometry})
}
func HeadersEncodeBytes(profile HeaderProfile, payload Payload) (Headers, error) {
	return operationInto[Headers]("headers.encode_bytes", struct {
		Profile HeaderProfile `json:"profile"`
		Payload Payload       `json:"payload"`
	}{profile, payload})
}
func HeadersDecodeBytes(profile HeaderProfile, headers Headers) (*Payload, error) {
	return operationInto[*Payload]("headers.decode_bytes", struct {
		Profile HeaderProfile `json:"profile"`
		Headers Headers       `json:"headers"`
	}{profile, headers})
}
func HeadersEncodeFinalized(profile HeaderProfile, text string, context *HeaderContext) (Headers, error) {
	return operationInto[Headers]("headers.encode_finalized", struct {
		Profile HeaderProfile  `json:"profile"`
		Text    string         `json:"text"`
		Context *HeaderContext `json:"context,omitempty"`
	}{profile, text, context})
}
func HeadersDecodeFinalized(profile HeaderProfile, headers Headers) (*FinalizedHeaders, error) {
	return operationInto[*FinalizedHeaders]("headers.decode_finalized", struct {
		Profile HeaderProfile `json:"profile"`
		Headers Headers       `json:"headers"`
	}{profile, headers})
}
func PayloadEncode(profile PayloadProfile, report FramedReport) (string, error) {
	return operationInto[string]("payload.encode", struct {
		Profile PayloadProfile `json:"profile"`
		Report  FramedReport   `json:"report"`
	}{profile, report})
}
func PayloadDecode(profile PayloadProfile, text string, geometry ReportGeometry) (*FramedReport, error) {
	return operationInto[*FramedReport]("payload.decode", struct {
		Profile PayloadProfile `json:"profile"`
		Text    string         `json:"text"`
		ReportGeometry
	}{profile, text, geometry})
}
func PayloadEncodeBytes(profile PayloadProfile, bytes []byte, seeds [2]uint32) (string, error) {
	return operationInto[string]("payload.encode_bytes", struct {
		Profile PayloadProfile `json:"profile"`
		Bytes   Bytes          `json:"bytes"`
		Seeds   [2]uint32      `json:"seeds"`
	}{profile, Bytes(bytes), seeds})
}
func PayloadDecodeBytes(profile PayloadProfile, text string) (*Payload, error) {
	return operationInto[*Payload]("payload.decode_bytes", struct {
		Profile PayloadProfile `json:"profile"`
		Text    string         `json:"text"`
	}{profile, text})
}
