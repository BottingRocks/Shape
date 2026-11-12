package shape

import "encoding/json"

// LoadedArtifact owns one immutable, validated native decode artifact. It can
// be shared by goroutines; Close waits for active calls. Loading/compiling does
// not persist files or enable a disk cache. Its zero value is closed.
type LoadedArtifact struct{ generator *Generator }

// LoadArtifact parses and validates executable decode JSON once, outside loops.
func LoadArtifact(document []byte) (*LoadedArtifact, error) {
	g, err := Load(document)
	if err != nil {
		return nil, err
	}
	return &LoadedArtifact{generator: g}, nil
}

func (a *LoadedArtifact) handle() (*Generator, error) {
	if a == nil || a.generator == nil {
		return nil, &Error{Code: CodeInvalidRequest, Message: "artifact is closed"}
	}
	return a.generator, nil
}
func (a *LoadedArtifact) Close() error {
	if a == nil || a.generator == nil {
		return nil
	}
	return a.generator.Close()
}

// JSON explicitly exports an independent document copy for caller persistence.
// It cannot mutate the loaded artifact. It fails after Close.
func (a *LoadedArtifact) JSON() (json.RawMessage, error) {
	g, err := a.handle()
	if err != nil {
		return nil, err
	}
	return g.Call("artifact", nil)
}
func Decode(a *LoadedArtifact, capture Capture, options DecodeOptions) (*Decoded, error) {
	g, err := a.handle()
	if err != nil {
		return nil, err
	}
	return g.Decode(capture, options)
}
func Explain(a *LoadedArtifact, decoded *Decoded, signal string, options ExplainOptions) (*Signal, error) {
	g, err := a.handle()
	if err != nil {
		return nil, err
	}
	return g.Explain(decoded, signal, options)
}
func Encode(a *LoadedArtifact, decoded *Decoded, session json.RawMessage) (*Capture, error) {
	g, err := a.handle()
	if err != nil {
		return nil, err
	}
	return g.Encode(EncodeRequest{Decoded: decoded, Session: session})
}

// NamedSignals preserves exact numbers in named fields via json.Number.
type NamedSignals map[string]map[string]any

func EncodeSignals(a *LoadedArtifact, signals NamedSignals, session json.RawMessage) (*Capture, error) {
	g, err := a.handle()
	if err != nil {
		return nil, err
	}
	return g.Encode(EncodeRequest{Signals: signals, Session: session})
}
func Schema(a *LoadedArtifact) (json.RawMessage, error) {
	g, err := a.handle()
	if err != nil {
		return nil, err
	}
	return g.Schema()
}
func (a *LoadedArtifact) Decode(capture Capture, options DecodeOptions) (*Decoded, error) {
	return Decode(a, capture, options)
}
func (a *LoadedArtifact) Explain(decoded *Decoded, signal string, options ExplainOptions) (*Signal, error) {
	return Explain(a, decoded, signal, options)
}
func (a *LoadedArtifact) Encode(decoded *Decoded, session json.RawMessage) (*Capture, error) {
	return Encode(a, decoded, session)
}
func (a *LoadedArtifact) EncodeSignals(signals NamedSignals, session json.RawMessage) (*Capture, error) {
	return EncodeSignals(a, signals, session)
}
func (a *LoadedArtifact) Schema() (json.RawMessage, error) { return Schema(a) }
