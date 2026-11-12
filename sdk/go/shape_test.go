package shape_test

import (
	"encoding/json"
	"errors"
	"math/big"
	"os"
	"reflect"
	"strings"
	"sync"
	"testing"

	"shape"
)

func TestVersion(t *testing.T) {
	v := shape.Version()
	if v == "" {
		t.Fatal("empty version")
	}
	value, err := shape.Call("version", nil)
	if err != nil {
		t.Fatal(err)
	}
	var out struct {
		Version  string   `json:"version"`
		Commands []string `json:"commands"`
	}
	if err := json.Unmarshal(value, &out); err != nil {
		t.Fatal(err)
	}
	if out.Version != v {
		t.Fatalf("version command %q != shape_version %q", out.Version, v)
	}
	if len(out.Commands) == 0 {
		t.Fatal("no commands listed")
	}
}

func wantCode(t *testing.T, err error, code string) {
	t.Helper()
	var serr *shape.Error
	if !errors.As(err, &serr) {
		t.Fatalf("want *shape.Error with code %s, got %v", code, err)
	}
	if serr.Code != code {
		t.Fatalf("want code %s, got %s (%s)", code, serr.Code, serr.Message)
	}
	if serr.Message == "" {
		t.Fatal("empty error message")
	}
}

func TestErrorEnvelopes(t *testing.T) {
	_, err := shape.Call("no.such.command", nil)
	wantCode(t, err, shape.CodeUnknownCommand)

	_, err = shape.Call("deob", map[string]any{})
	wantCode(t, err, shape.CodeInvalidRequest)

	_, err = shape.Call("deob", json.RawMessage(`{`))
	wantCode(t, err, shape.CodeInvalidRequest)

	_, err = shape.Call("cleanup.thread", map[string]any{"source": 7})
	wantCode(t, err, shape.CodeInvalidRequest)

	_, err = shape.Load([]byte("not json"))
	wantCode(t, err, shape.CodeFailed)

	_, err = shape.Load(nil)
	if err == nil {
		t.Fatal("empty artifact loaded")
	}
}

func TestCleanupThread(t *testing.T) {
	out, err := shape.CleanupThread("function f(a) { let $SSA_1 = a; return $SSA_1; }")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.Code, "return") {
		t.Fatalf("cleaned code lacks return: %q", out.Code)
	}
	if strings.Contains(out.Code, "$SSA_1") {
		t.Fatalf("binding not inlined: %q", out.Code)
	}
}

func TestCleanupThreadConcurrent(t *testing.T) {
	var wg sync.WaitGroup
	errs := make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, err := shape.CleanupThread("function f(a) { let $SSA_1 = a; return $SSA_1; }"); err != nil {
				errs <- err
			}
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Fatal(err)
	}
}

// Generator tests need a local decode artifact and a matching capture:
// SHAPE_TEST_ARTIFACT=/path/artifact.json SHAPE_TEST_HEADERS=/path/headers.json.
func loadFixture(t *testing.T) (*shape.Generator, []byte) {
	t.Helper()
	artifactPath, headersPath := os.Getenv("SHAPE_TEST_ARTIFACT"), os.Getenv("SHAPE_TEST_HEADERS")
	if artifactPath == "" || headersPath == "" {
		t.Skip("set SHAPE_TEST_ARTIFACT and SHAPE_TEST_HEADERS to run generator tests")
	}
	artifact, err := os.ReadFile(artifactPath)
	if err != nil {
		t.Fatal(err)
	}
	headers, err := os.ReadFile(headersPath)
	if err != nil {
		t.Fatal(err)
	}
	gen, err := shape.Load(artifact)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { gen.Close() })
	return gen, headers
}

func deviceMemory(t *testing.T, d *shape.Decoded) {
	t.Helper()
	got, ok := d.PositionalSignals["DeviceMemory"]["deviceMemory"].(json.Number)
	if !ok || got.String() != "32" {
		t.Fatalf("DeviceMemory.deviceMemory = %v, want 32", d.PositionalSignals["DeviceMemory"]["deviceMemory"])
	}
}

// findLargeNumber reports the first integer with |n| > 2^53 in a decoded
// value. Numbers in any-typed fields are json.Number; a float64 here would
// mean the binding lost the exact digits.
func findLargeNumber(value any) (string, bool) {
	limit := new(big.Int).Lsh(big.NewInt(1), 53)
	switch v := value.(type) {
	case json.RawMessage:
		var decoded any
		dec := json.NewDecoder(strings.NewReader(string(v)))
		dec.UseNumber()
		if err := dec.Decode(&decoded); err == nil {
			return findLargeNumber(decoded)
		}
	case json.Number:
		number, ok := new(big.Int).SetString(v.String(), 10)
		if ok && (number.Cmp(limit) > 0 || number.Cmp(new(big.Int).Neg(limit)) < 0) {
			return v.String(), true
		}
	case map[string]any:
		for _, item := range v {
			if text, ok := findLargeNumber(item); ok {
				return text, true
			}
		}
	case []any:
		for _, item := range v {
			if text, ok := findLargeNumber(item); ok {
				return text, true
			}
		}
	}
	return "", false
}

func TestGeneratorDecodePreservesLargeIntegers(t *testing.T) {
	gen, headers := loadFixture(t)
	decoded, err := gen.Decode(json.RawMessage(headers), shape.DecodeOptions{Metadata: true})
	if err != nil {
		t.Fatal(err)
	}
	for _, signal := range decoded.Metadata.Signals {
		if _, ok := findLargeNumber(signal.Fields); ok {
			return
		}
		if _, ok := findLargeNumber(signal.Metadata); ok {
			return
		}
	}
	// Compare against the native JSON to distinguish small fixture values from
	// a binding regression (typed fields must retain every native integer).
	raw, err := gen.Call("decode", map[string]any{"capture": json.RawMessage(headers), "metadata": true})
	if err != nil {
		t.Fatal(err)
	}
	var native map[string]any
	dec := json.NewDecoder(strings.NewReader(string(raw)))
	dec.UseNumber()
	if err := dec.Decode(&native); err != nil {
		t.Fatal(err)
	}
	if _, ok := findLargeNumber(native["metadata"].(map[string]any)["signals"]); ok {
		t.Fatal("typed metadata lost an integer above 2^53 present in native JSON")
	}
	t.Skip("fixture has no integer above 2^53 in signal metadata; precision is covered by boundary tests")
}

func TestGeneratorDecodeExplain(t *testing.T) {
	gen, headers := loadFixture(t)

	decoded, err := gen.Decode(json.RawMessage(headers), shape.DecodeOptions{})
	if err != nil {
		t.Fatal(err)
	}
	deviceMemory(t, decoded)
	if decoded.Metadata.Decoded == 0 || decoded.Metadata.Decoded != len(decoded.PositionalSignals) {
		t.Fatalf("metadata.decoded = %d, positional signals = %d", decoded.Metadata.Decoded, len(decoded.PositionalSignals))
	}

	// A typed Capture decodes the same.
	var capture shape.Capture
	if err := json.Unmarshal(headers, &capture); err != nil {
		t.Fatal(err)
	}
	again, err := gen.Decode(capture, shape.DecodeOptions{})
	if err != nil {
		t.Fatal(err)
	}
	deviceMemory(t, again)

	signal, err := gen.Explain(decoded, "DeviceMemory", shape.ExplainOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if signal.Status != "decoded" {
		t.Fatalf("explain status = %q, want decoded", signal.Status)
	}

	_, err = gen.Explain(decoded, "NoSuchSignal", shape.ExplainOptions{})
	if err == nil {
		t.Fatal("explain of unknown signal succeeded")
	}

	schema, err := gen.Schema()
	if err != nil {
		t.Fatal(err)
	}
	if !json.Valid(schema) || len(schema) < 2 {
		t.Fatal("invalid schema")
	}

	// Re-encoding the full payload reproduces the capture byte for byte.
	full, err := gen.Decode(json.RawMessage(headers), shape.DecodeOptions{Metadata: true})
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := gen.Encode(shape.EncodeRequest{Decoded: full})
	if err != nil {
		t.Fatal(err)
	}
	var want shape.Capture
	if err := json.Unmarshal(headers, &want); err != nil {
		t.Fatal(err)
	}
	if encoded.SiteID != want.SiteID || !reflect.DeepEqual(encoded.Headers, want.Headers) {
		t.Fatal("encoded capture differs from the fixture")
	}
	// A bare edit is encoded from fresh prefixes and decodes back.
	full.PositionalSignals["DeviceMemory"]["deviceMemory"] = json.Number("8")
	fullEdited, err := gen.Encode(shape.EncodeRequest{Decoded: full})
	if err != nil {
		t.Fatal(err)
	}
	fullAgain, err := gen.Decode(fullEdited, shape.DecodeOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if got := fullAgain.PositionalSignals["DeviceMemory"]["deviceMemory"]; got != json.Number("8") {
		t.Fatalf("full decoded edit ignored: got %v, want 8", got)
	}
	edited, err := gen.Encode(shape.EncodeRequest{Signals: full.PositionalSignals})
	if err != nil {
		t.Fatal(err)
	}
	again, err = gen.Decode(edited, shape.DecodeOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if got := again.PositionalSignals["DeviceMemory"]["deviceMemory"]; got != json.Number("8") {
		t.Fatalf("edited deviceMemory = %v, want 8", got)
	}

	_, err = gen.Call("no.such.method", nil)
	wantCode(t, err, shape.CodeUnknownCommand)

	_, err = gen.Call("decode", nil)
	wantCode(t, err, shape.CodeInvalidRequest)
}

func TestGeneratorConcurrentDecode(t *testing.T) {
	gen, headers := loadFixture(t)
	var wg sync.WaitGroup
	errs := make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			d, err := gen.Decode(json.RawMessage(headers), shape.DecodeOptions{})
			if err != nil {
				errs <- err
				return
			}
			if d.Metadata.Decoded == 0 || d.Metadata.Decoded != len(d.PositionalSignals) {
				errs <- errors.New("wrong decoded count")
			}
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Fatal(err)
	}
}

func TestGeneratorClose(t *testing.T) {
	gen, headers := loadFixture(t)
	if err := gen.Close(); err != nil {
		t.Fatal(err)
	}
	if err := gen.Close(); err != nil {
		t.Fatal(err)
	}
	_, err := gen.Decode(json.RawMessage(headers), shape.DecodeOptions{})
	wantCode(t, err, shape.CodeInvalidRequest)
}

func TestGeneratorConcurrentClose(t *testing.T) {
	gen, headers := loadFixture(t)
	start := make(chan struct{})
	errs := make(chan error, 8)
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			<-start
			_, err := gen.Decode(headers, shape.DecodeOptions{})
			if err != nil {
				var serr *shape.Error
				if !errors.As(err, &serr) || serr.Code != shape.CodeInvalidRequest || serr.Message != "generator is closed" {
					errs <- err
				}
			}
		}()
	}
	close(start)
	if err := gen.Close(); err != nil {
		t.Fatal(err)
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Error(err)
	}
}
