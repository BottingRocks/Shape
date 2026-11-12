// Operation namespaces group typed functions by domain. Flat functions remain
// available for idiomatic Go integrations and compatibility.
package shape

import "encoding/json"

// Artifact groups stateless artifact operations.
var Artifact artifactOperations

type artifactOperations struct{}

func (artifactOperations) Load(document []byte) (*LoadedArtifact, error) {
	return LoadArtifact(document)
}
func (artifactOperations) CompactSchema(artifact json.RawMessage) (json.RawMessage, error) {
	return CompactSchema(artifact)
}
func (artifactOperations) CompileDocument(source string, options ArtifactOptions) (json.RawMessage, error) {
	return CompileArtifactWithOptions(source, options)
}

// Kernel groups stateless kernel operations.
var Kernel kernelOperations

type kernelOperations struct{}

func (kernelOperations) Load(document []byte) (*LoadedKernel, error)     { return LoadKernel(document) }
func (kernelOperations) ToJSON(artifact json.RawMessage) (string, error) { return KernelJSON(artifact) }
func (kernelOperations) Document(artifact json.RawMessage) (json.RawMessage, error) {
	return KernelDocument(artifact)
}
func (kernelOperations) Show(source string, opts KernelShowOptions) (json.RawMessage, error) {
	return KernelShow(source, opts)
}

// Ops groups stateless ops operations.
var Ops opsOperations

type opsOperations struct{ Classify classifyOperations }

func (opsOperations) List(source string) (json.RawMessage, error) { return OpsList(source) }
func (opsOperations) Show(source string, op uint32) (json.RawMessage, error) {
	return OpsShow(source, op)
}

type classifyOperations struct{}

func (classifyOperations) Source(source string) (*Classification, error) {
	return OpsClassifySource(source)
}
func (classifyOperations) Report(source string) (*Classification, error) {
	return OpsClassifyReport(source)
}
func (classifyOperations) Template(source string, handlerIndex uint32) (*ClassifiedOp, error) {
	return OpsClassifyTemplate(source, handlerIndex)
}
func (classifyOperations) Seeds(seeds []Classification) ([]KindSummary, error) {
	return OpsClassifySeeds(seeds)
}

// Trace groups stateless trace operations.
var Trace traceOperations

type traceOperations struct{ Corpus traceCorpusOperations }

func (traceOperations) Program(source string, options ProgramOptions) (*Program, error) {
	return TraceProgramWithOptions(source, options)
}
func (traceOperations) ProgramWithArtifact(source string, artifact json.RawMessage, options ProgramOptions) (*Program, error) {
	return TraceProgramWithArtifact(source, artifact, options)
}
func (traceOperations) Thread(req TraceThreadRequest) (*Thread, error) { return TraceThread(req) }

type traceCorpusOperations struct{}

func (traceCorpusOperations) Load(source string, options CorpusSourceOptions) (json.RawMessage, error) {
	return TraceCorpusLoad(source, options)
}
func (traceCorpusOperations) Source(source string, options CorpusSourceOptions) ([]Fingerprint, error) {
	return TraceCorpusSource(source, options)
}
func (traceCorpusOperations) Reachable(artifact json.RawMessage, options ReachableOptions) ([]Fingerprint, error) {
	return TraceCorpusReachable(artifact, options)
}
func (traceCorpusOperations) WalkSeed(seed string, options CorpusWalkOptions) (json.RawMessage, error) {
	return TraceCorpusWalkSeed(seed, options)
}
func (traceCorpusOperations) Fingerprint(artifact json.RawMessage, options FingerprintOptions) (*ThreadFingerprint, error) {
	return TraceCorpusFingerprint(artifact, options)
}
func (traceCorpusOperations) Compare(baseline, current json.RawMessage) (*CorpusComparison, error) {
	return TraceCorpusCompare(baseline, current)
}
func (traceCorpusOperations) CompareInline(outlined, inline json.RawMessage) ([]string, error) {
	return TraceCorpusCompareInline(outlined, inline)
}

// CleanupOperations groups stateless cleanupoperations operations.
var CleanupOperations cleanupOperations

type cleanupOperations struct{}

func (cleanupOperations) Thread(source string) (*Cleanup, error) { return CleanupThread(source) }

// Naming groups stateless naming operations.
var Naming namingOperations

type namingOperations struct{ Corpus namingCorpusOperations }

func (namingOperations) Threads(source string, opts NamingThreadsOptions) (*Named, error) {
	return NamingThreads(source, opts)
}
func (namingOperations) Collectors(source string, opts NamingCollectorsOptions) (*Named, error) {
	return NamingCollectors(source, opts)
}
func (namingOperations) Plan(source string) (json.RawMessage, error) { return NamingPlan(source) }
func (namingOperations) Inventory(source string) (*Inventory, error) { return NamingInventory(source) }

type namingCorpusOperations struct{}

func (namingCorpusOperations) Analyze(sources []NamedSource) ([]NamingAnalysis, error) {
	return NamingCorpusAnalyze(sources)
}
func (namingCorpusOperations) Compare(baseline, current []NamedSource) ([]NamingDifference, error) {
	return NamingCorpusCompare(baseline, current)
}

// Signals groups stateless signals operations.
var Signals signalOperations

type signalOperations struct{}

func (signalOperations) Schema(source string, opts SignalsSchemaOptions) (json.RawMessage, error) {
	return SignalsSchema(source, opts)
}
func (signalOperations) Show(source string) (json.RawMessage, error)   { return SignalsShow(source) }
func (signalOperations) Inputs(source string) (json.RawMessage, error) { return SignalsInputs(source) }
func (signalOperations) Contracts(source string) (json.RawMessage, error) {
	return SignalsContracts(source)
}
func (signalOperations) Catalog() (json.RawMessage, error)             { return SignalsCatalog() }
func (signalOperations) Describe(name string) (json.RawMessage, error) { return SignalsDescribe(name) }
func (signalOperations) CompactSchema(source string, options SourceSchemaOptions) (json.RawMessage, error) {
	return SignalsCompactSchema(source, options)
}

// Session groups stateless session operations.
var Session sessionOperations

type sessionOperations struct{}

func (sessionOperations) FromInitializer(init string) (json.RawMessage, error) {
	return SessionFromInitializer(init)
}

// Seeds groups stateless seeds operations.
var Seeds seedOperations

type seedOperations struct{}

func (seedOperations) Parse(bytes []byte) (*SeedScript, error) { return SeedsParse(bytes) }
func (seedOperations) ParseLauncher(bytes []byte) (*Launcher, error) {
	return SeedsParseLauncher(bytes)
}
func (seedOperations) ParseKernel(bytes []byte) (*KernelSummary, error) {
	return SeedsParseKernel(bytes)
}
func (seedOperations) ParseURL(input string) (string, error) { return SeedsParseURL(input) }
func (seedOperations) ResolveURL(reference, base string) (string, error) {
	return SeedsResolveURL(reference, base)
}
func (seedOperations) Token(reference, param string) (string, error) {
	return SeedsToken(reference, param)
}
func (seedOperations) Sites() ([]Site, error)                            { return SeedsSites() }
func (seedOperations) Site(name string) (*Site, error)                   { return SeedsSite(name) }
func (seedOperations) Client(config ClientConfig) (*ClientConfig, error) { return SeedsClient(config) }
func (seedOperations) Fetch(url string, options FetchOptions) (*FetchedSeed, error) {
	return SeedsFetch(url, options)
}
func (seedOperations) FetchDirect(url string, options FetchOptions) (*FetchedSeed, error) {
	return SeedsFetchDirect(url, options)
}
func (seedOperations) Import(directory, seed string, launcher, kernel []byte) (*StoreOutcome, error) {
	return SeedsImport(directory, seed, launcher, kernel)
}

// HeaderOperations groups stateless headeroperations operations.
var HeaderOperations headerOperations

type headerOperations struct{}

func (headerOperations) Codec(profile HeaderProfile) (*HeaderProfile, error) {
	return HeadersCodec(profile)
}
func (headerOperations) Parse(text string) (Headers, error) { return HeadersParse(text) }
func (headerOperations) Encode(profile HeaderProfile, report FramedReport) (Headers, error) {
	return HeadersEncode(profile, report)
}
func (headerOperations) Decode(profile HeaderProfile, headers Headers, geometry ReportGeometry) (*FramedReport, error) {
	return HeadersDecode(profile, headers, geometry)
}
func (headerOperations) EncodeBytes(profile HeaderProfile, payload Payload) (Headers, error) {
	return HeadersEncodeBytes(profile, payload)
}
func (headerOperations) DecodeBytes(profile HeaderProfile, headers Headers) (*Payload, error) {
	return HeadersDecodeBytes(profile, headers)
}
func (headerOperations) EncodeFinalized(profile HeaderProfile, text string, context *HeaderContext) (Headers, error) {
	return HeadersEncodeFinalized(profile, text, context)
}
func (headerOperations) DecodeFinalized(profile HeaderProfile, headers Headers) (*FinalizedHeaders, error) {
	return HeadersDecodeFinalized(profile, headers)
}

// PayloadOperations groups stateless payloadoperations operations.
var PayloadOperations payloadOperations

type payloadOperations struct{}

func (payloadOperations) Codec(profile PayloadProfile) (*PayloadProfile, error) {
	return PayloadCodec(profile)
}
func (payloadOperations) Encode(profile PayloadProfile, report FramedReport) (string, error) {
	return PayloadEncode(profile, report)
}
func (payloadOperations) Decode(profile PayloadProfile, text string, geometry ReportGeometry) (*FramedReport, error) {
	return PayloadDecode(profile, text, geometry)
}
func (payloadOperations) EncodeBytes(profile PayloadProfile, bytes []byte, seeds [2]uint32) (string, error) {
	return PayloadEncodeBytes(profile, bytes, seeds)
}
func (payloadOperations) DecodeBytes(profile PayloadProfile, text string) (*Payload, error) {
	return PayloadDecodeBytes(profile, text)
}
func (artifactOperations) Compile(source string, options ArtifactOptions) (*LoadedArtifact, error) {
	return CompileLoadedArtifact(source, options)
}

func (classifyOperations) ReportDocument(report json.RawMessage) (*Classification, error) {
	return OpsClassifyReportDocument(report)
}
func (classifyOperations) TemplateDocument(source string, template json.RawMessage) (*ClassifiedOp, error) {
	return OpsClassifyTemplateDocument(source, template)
}
