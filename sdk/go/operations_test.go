package shape

import (
	"bytes"
	"encoding/json"
	"errors"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"testing"
)

func TestOperationOptions(t *testing.T) {
	disabled, err := json.Marshal(ProgramOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(disabled, []byte(`"cache":{"mode":"disabled"}`)) {
		t.Fatalf("cache default: %s", disabled)
	}
	for _, key := range []string{`"jobs"`, `"thread_cleanup"`, `"no_cleanup"`} {
		if bytes.Contains(disabled, []byte(key)) {
			t.Fatalf("zero options override native cleanup defaults: %s", disabled)
		}
	}
	no, yes, jobs := false, true, 2
	explicit, err := json.Marshal(ProgramOptions{
		Cache: CachePolicy{Mode: CacheDirectory, Directory: "cache"}, ThreadCleanup: &no, NoCleanup: &yes, Jobs: &jobs,
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{`"mode":"directory"`, `"directory":"cache"`, `"thread_cleanup":false`, `"no_cleanup":true`, `"jobs":2`} {
		if !bytes.Contains(explicit, []byte(field)) {
			t.Fatalf("lost explicit option %s: %s", field, explicit)
		}
	}
	artifact, err := json.Marshal(ArtifactOptions{Session: json.RawMessage(`{}`), SiteID: "public"})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(artifact, []byte(`"cache":{"mode":"disabled"}`)) {
		t.Fatalf("artifact cache: %s", artifact)
	}
}

func TestBytesAndPackPrecision(t *testing.T) {
	for _, value := range []Bytes{nil, {}, {0, 128, 255}} {
		raw, err := json.Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		if len(raw) == 0 || raw[0] != '[' {
			t.Fatalf("bytes not an array: %s", raw)
		}
		var got Bytes
		if err := json.Unmarshal(raw, &got); err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(got, value) {
			t.Fatal("bytes changed")
		}
	}
	for _, raw := range []string{`"AA=="`, `[256]`, `[-1]`, `[1.5]`} {
		var b Bytes
		if err := json.Unmarshal([]byte(raw), &b); err == nil {
			t.Fatalf("accepted invalid bytes %s", raw)
		}
	}
	for _, bits := range []uint64{1 << 63, math.MaxUint64, 0x7ff8000000000001} {
		value := PackValue{Kind: PackNumber, NumberBits: bits}
		raw, err := json.Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		var recovered PackValue
		if err := json.Unmarshal(raw, &recovered); err != nil {
			t.Fatal(err)
		}
		if recovered.NumberBits != bits {
			t.Fatalf("bits rounded: %x", recovered.NumberBits)
		}
	}
	value := PackValue{Kind: PackDate, Date: math.MaxUint64}
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), "18446744073709551615") {
		t.Fatal(string(raw))
	}
}

func TestPortablePrimitives(t *testing.T) {
	version, err := NativeVersion()
	if err != nil {
		t.Fatal(err)
	}
	if version.ContractVersion != 2 || version.Version == "" || len(version.Commands) < 60 {
		t.Fatalf("wrong library: %+v", version)
	}
	source := PackValue{Kind: PackArray, Array: []PackValue{
		{Kind: PackBool, Bool: true}, {Kind: PackString, Text: "public sample"},
		{Kind: PackNumber, NumberBits: math.Float64bits(-17)},
		{Kind: PackNumber, NumberBits: 1 << 63},
		{Kind: PackBinary, Binary: Bytes{0, 128, 255}},
	}}
	packed, err := Pack(source, SerializerOptions{})
	if err != nil {
		t.Fatal(err)
	}
	got, err := UnpackCanonical(packed, SerializerOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(*got, source) {
		t.Fatalf("pack changed values: %#v", got)
	}
	rich, err := Unpack(packed, SerializerOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if !rich.Canonical || !bytes.Equal(rich.OriginalBytes, packed) {
		t.Fatalf("unpack lost original bytes: %+v", rich)
	}
	resolved, err := SeedsResolveURL("kernel.js?seed=public-token", "https://example.test/base/")
	if err != nil {
		t.Fatal(err)
	}
	if resolved != "https://example.test/base/kernel.js?seed=public-token" {
		t.Fatal(resolved)
	}
	token, err := SeedsToken(resolved, "")
	if err != nil {
		t.Fatal(err)
	}
	if token != "public-token" {
		t.Fatal(token)
	}
	headers, err := HeadersParse("x-sample-a: sample\n")
	if err != nil {
		t.Fatal(err)
	}
	if headers["x-sample-a"] != "sample" {
		t.Fatal(headers)
	}
	comparison, err := TraceCorpusCompare(json.RawMessage(`{}`), json.RawMessage(`{}`))
	if err != nil {
		t.Fatal(err)
	}
	if len(comparison.Changes) != 0 || len(comparison.Regressions) != 0 {
		t.Fatal(comparison)
	}
	sites, err := SeedsSites()
	if err != nil {
		t.Fatal(err)
	}
	if len(sites) == 0 {
		t.Fatal("empty site registry")
	}
	if _, err := SignalsDescribe("DeviceMemory"); err != nil {
		t.Fatal(err)
	}
}

func TestOperationErrors(t *testing.T) {
	for _, tc := range []struct {
		op      string
		request any
		code    string
	}{
		{"does.not.exist", nil, CodeUnknownCommand},
		{"seeds.parse_url", map[string]string{"input": "file:///private"}, CodeFailed},
		{"pack", map[string]any{"value": map[string]string{"kind": "not-a-kind"}}, CodeInvalidRequest},
		{"trace.program", map[string]any{"source": "", "cache": map[string]string{"mode": "bogus"}}, CodeInvalidRequest},
		{"signals.describe", map[string]string{"name": "NoSuchCanonicalSignal"}, CodeInvalidRequest},
		{"version\x00ignored", nil, CodeInvalidRequest},
	} {
		_, err := CallOperation(tc.op, tc.request)
		var sdk *Error
		if !errors.As(err, &sdk) || sdk.Code != tc.code {
			t.Errorf("%s: %v, want %s", tc.op, err, tc.code)
		}
	}
}

func TestPayloadAndHeaders(t *testing.T) {
	profile := PayloadProfile{
		Format: "shape-byte-codec/1", Cipher: CipherProfile{IVs: [4]uint32{1, 2, 3, 4}, Counter: 0, DoubleRounds: 5},
		Alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/", Keys: [8]uint32{1, 2, 3, 4, 5, 6, 7, 8},
	}
	codec, err := PayloadCodec(profile)
	if err != nil {
		t.Fatal(err)
	}
	plain := []byte("public synthetic bytes")
	text, err := PayloadEncodeBytes(*codec, plain, [2]uint32{1, 2})
	if err != nil {
		t.Fatal(err)
	}
	payload, err := PayloadDecodeBytes(*codec, text)
	if err != nil {
		t.Fatal(err)
	}
	if payload.Data.Kind != "raw" || !bytes.Equal(payload.Data.Bytes, plain) {
		t.Fatalf("payload mismatch: %+v", payload)
	}
	hp := HeaderProfile{PayloadProfile: profile, HeaderName: "x-public-a"}
	headers, err := HeadersEncodeBytes(hp, *payload)
	if err != nil {
		t.Fatal(err)
	}
	recovered, err := HeadersDecodeBytes(hp, headers)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(recovered.Data.Bytes, plain) {
		t.Fatal("header bytes changed")
	}
}

func TestLoadedArtifactLifecycle(t *testing.T) {
	artifact, err := os.ReadFile(filepath.Join("..", "c", "tests", "fixtures", "artifact.json"))
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join("..", "c", "tests", "fixtures", "capture.json"))
	if err != nil {
		t.Fatal(err)
	}
	var capture Capture
	if err := json.Unmarshal(data, &capture); err != nil {
		t.Fatal(err)
	}
	a, err := LoadArtifact(artifact)
	if err != nil {
		t.Fatal(err)
	}
	defer a.Close()
	decoded, err := Decode(a, capture, DecodeOptions{Verify: true})
	if err != nil {
		t.Fatal(err)
	}
	fields := decoded.PositionalSignals["DeviceMemory"]
	if fields == nil {
		t.Fatal("missing DeviceMemory")
	}
	fields["deviceMemory"] = json.Number("8")
	encoded, err := Encode(a, decoded, nil)
	if err != nil {
		t.Fatal(err)
	}
	check, err := Decode(a, *encoded, DecodeOptions{Verify: true})
	if err != nil {
		t.Fatal(err)
	}
	if check.PositionalSignals["DeviceMemory"]["deviceMemory"] != json.Number("8") {
		t.Fatal("edit lost")
	}
	if _, err := Explain(a, check, "DeviceMemory", ExplainOptions{}); err != nil {
		t.Fatal(err)
	}
	exported, err := a.JSON()
	if err != nil {
		t.Fatal(err)
	}
	exported[0] = '!'
	if _, err := Schema(a); err != nil {
		t.Fatal(err)
	}
	var wg sync.WaitGroup
	failures := make(chan error, 20)
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := Decode(a, capture, DecodeOptions{})
			if err != nil {
				var e *Error
				if !errors.As(err, &e) || e.Code != CodeInvalidRequest {
					failures <- err
				}
			}
		}()
	}
	wg.Add(1)
	go func() {
		defer wg.Done()
		if err := a.Close(); err != nil {
			failures <- err
		}
	}()
	wg.Wait()
	close(failures)
	for err := range failures {
		t.Error(err)
	}
	if err := a.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := Schema(a); err == nil {
		t.Fatal("schema after close succeeded")
	}
	if _, err := LoadArtifact([]byte(`{}`)); err == nil {
		t.Fatal("invalid artifact loaded")
	}
	var zero LoadedArtifact
	if err := zero.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := Schema(&zero); err == nil {
		t.Fatal("zero artifact usable")
	}
	if _, err := Schema(nil); err == nil {
		t.Fatal("nil artifact usable")
	}
}

func TestKernelCacheAndMemoryReuse(t *testing.T) {
	source, err := os.ReadFile(filepath.Join("..", "..", "tests", "fixtures", "javascript-vm-prefix.raw.js"))
	if err != nil {
		t.Fatal(err)
	}
	before := os.Environ()
	dir := filepath.Join(t.TempDir(), "cache")
	options := CorpusSourceOptions{Name: "public.js", Cache: CachePolicy{Mode: CacheDirectory, Directory: dir}}
	document, err := TraceCorpusLoad(string(source), options)
	if err != nil {
		t.Fatal(err)
	}
	cacheFile := filepath.Join(dir, "public.js.kernel.json")
	if _, err := os.Stat(cacheFile); err != nil {
		t.Fatalf("explicit cache not written: %v", err)
	}
	kernel, err := LoadKernel(document)
	if err != nil {
		t.Fatal(err)
	}
	defer kernel.Close()
	if err := os.RemoveAll(dir); err != nil {
		t.Fatal(err)
	}
	traced, err := kernel.TraceProgram(string(source), ProgramOptions{})
	if err != nil {
		t.Fatal(err)
	}
	noCleanup, threadCleanup := false, true
	explicit, err := TraceProgramWithOptions(string(source), ProgramOptions{ThreadCleanup: &threadCleanup, NoCleanup: &noCleanup})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(*traced, *explicit) {
		t.Fatal("zero options differ from explicit full cleanup policy")
	}
	if _, err := os.Stat(dir); !os.IsNotExist(err) {
		t.Fatalf("memory reuse recreated disk cache: %v", err)
	}
	exported, err := kernel.JSON()
	if err != nil {
		t.Fatal(err)
	}
	if !json.Valid(exported) {
		t.Fatal("kernel export invalid")
	}
	if _, err := kernel.TraceProgram(string(source)+"\n// changed", ProgramOptions{}); err == nil {
		t.Fatal("kernel accepted different source identity")
	}
	if err := kernel.Close(); err != nil {
		t.Fatal(err)
	}
	if err := kernel.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := kernel.JSON(); err == nil {
		t.Fatal("closed kernel exported")
	}
	if !reflect.DeepEqual(before, os.Environ()) {
		t.Fatal("SDK modified environment")
	}
	invalid, err := LoadKernel([]byte(`{}`))
	if err == nil {
		invalid.Close()
		t.Fatal("invalid kernel loaded")
	}
	if err := os.MkdirAll(dir, 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cacheFile, []byte(`{}`), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := TraceCorpusLoad(string(source), options); err == nil {
		t.Fatal("invalid explicitly selected cache accepted")
	}
}

func TestTypedReportFraming(t *testing.T) {
	document, err := os.ReadFile(filepath.Join("..", "..", "tests", "fixtures", "sdk", "generator", "profile.json"))
	if err != nil {
		t.Fatal(err)
	}
	var profile HeaderProfile
	if err := json.Unmarshal(document, &profile); err != nil {
		t.Fatal(err)
	}
	report := FramedReport{
		Seeds:             [2]uint32{4294967295, 2},
		PositionalSignals: []*ReportEntry{nil, {Values: []PackValue{{Kind: PackNumber, NumberBits: 0x43e0000000000000}}, Extra: Bytes{128, 255}}},
	}
	headers, err := HeadersEncode(profile, report)
	if err != nil {
		t.Fatal(err)
	}
	geometry := ReportGeometry{PositionalCount: 2}
	recovered, err := HeadersDecode(profile, headers, geometry)
	if err != nil {
		t.Fatal(err)
	}
	if len(recovered.PositionalSignals) != 2 || recovered.PositionalSignals[0] != nil {
		t.Fatal("missing sentinel changed")
	}
	if !reflect.DeepEqual(recovered.PositionalSignals[1], report.PositionalSignals[1]) {
		t.Fatal("typed entry changed")
	}
	text, err := PayloadEncode(profile.PayloadProfile, report)
	if err != nil {
		t.Fatal(err)
	}
	again, err := PayloadDecode(profile.PayloadProfile, text, geometry)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(again.PositionalSignals, report.PositionalSignals) {
		t.Fatal("payload report changed")
	}
	finalized, err := HeadersDecodeFinalized(profile, headers)
	if err != nil {
		t.Fatal(err)
	}
	reframed, err := HeadersEncodeFinalized(profile, finalized.Text, &finalized.Context)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(reframed, headers) {
		t.Fatal("finalized headers changed")
	}
}

func TestCompileLoadedArtifact(t *testing.T) {
	base := filepath.Join("..", "..", "tests", "fixtures", "sdk", "decode")
	read := func(name string) []byte {
		t.Helper()
		data, err := os.ReadFile(filepath.Join(base, name))
		if err != nil {
			t.Fatal(err)
		}
		return data
	}
	source, session, data := read("program.js"), read("session.json"), read("headers.json")
	var capture Capture
	if err := json.Unmarshal(data, &capture); err != nil {
		t.Fatal(err)
	}
	loaded, err := Artifact.Compile(string(source), ArtifactOptions{Session: session, SiteID: capture.SiteID})
	if err != nil {
		t.Fatal(err)
	}
	defer loaded.Close()
	decoded, err := Decode(loaded, capture, DecodeOptions{Verify: true})
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := Encode(loaded, decoded, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(*encoded, capture) {
		t.Fatal("compiled artifact failed exact round trip")
	}
	exported, err := loaded.JSON()
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Artifact.Load(exported)
	if err != nil {
		t.Fatal(err)
	}
	defer restored.Close()
	a, err := Schema(loaded)
	if err != nil {
		t.Fatal(err)
	}
	b, err := Schema(restored)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(a, b) {
		t.Fatal("explicit export/reload changed schema")
	}
	if _, err := CompileLoadedArtifact("", ArtifactOptions{}); err == nil {
		t.Fatal("invalid compile succeeded")
	}
}
