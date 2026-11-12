package shape

import "encoding/json"

type ClassifiedOp struct {
	HandlerIndex uint32 `json:"handler_index"`
	Kind         string `json:"kind"`
	Nature       string `json:"nature"`
	Pops         uint32 `json:"pops"`
	Pushes       uint32 `json:"pushes"`
	Source       string `json:"source"`
}
type Classification struct {
	Variant string         `json:"variant"`
	Ops     []ClassifiedOp `json:"ops"`
}
type KindSummary struct {
	Kind     string      `json:"kind"`
	Nature   string      `json:"nature"`
	Scope    string      `json:"scope"`
	Handlers [][2]uint32 `json:"handlers"`
}

func OpsClassifySource(source string) (*Classification, error) {
	return operationInto[*Classification]("ops.classify.source", sourceRequest{source})
}

// OpsClassifyReport reconstructs the engine report from raw source, as the
// portable ABI cannot serialize a Rust PipelineReport.
func OpsClassifyReport(source string) (*Classification, error) {
	return operationInto[*Classification]("ops.classify.report", sourceRequest{source})
}
func OpsClassifyTemplate(source string, handlerIndex uint32) (*ClassifiedOp, error) {
	return operationInto[*ClassifiedOp]("ops.classify.template", struct {
		Source       string `json:"source"`
		HandlerIndex uint32 `json:"handler_index"`
	}{source, handlerIndex})
}
func OpsClassifySeeds(seeds []Classification) ([]KindSummary, error) {
	return operationInto[[]KindSummary]("ops.classify.seeds", struct {
		Seeds []Classification `json:"seeds"`
	}{seeds})
}

type ReachableOptions struct {
	Steps   int  `json:"steps,omitempty"`
	Jobs    int  `json:"jobs,omitempty"`
	Limit   *int `json:"limit,omitempty"`
	Cleanup bool `json:"cleanup,omitempty"`
}
type CorpusSourceOptions struct {
	Name  string      `json:"name,omitempty"`
	Cache CachePolicy `json:"cache"`
	ReachableOptions
}
type CorpusWalkOptions struct {
	Cache CachePolicy `json:"cache"`
	ReachableOptions
	SelfCheck bool   `json:"self_check,omitempty"`
	Audit     bool   `json:"audit,omitempty"`
	Branches  string `json:"branches,omitempty"`
}
type Function struct {
	Descriptor *int `json:"descriptor"`
	PC         int  `json:"pc"`
	State      int  `json:"state"`
}
type Fingerprint struct {
	Function Function        `json:"function"`
	Print    json.RawMessage `json:"print"`
	Code     *string         `json:"code"`
	Created  []Function      `json:"created"`
}
type FingerprintOptions struct {
	PC         int      `json:"pc"`
	State      int      `json:"state"`
	Descriptor *int     `json:"descriptor,omitempty"`
	Steps      int      `json:"steps,omitempty"`
	Parameters []string `json:"parameters,omitempty"`
	Cleanup    bool     `json:"cleanup,omitempty"`
}
type ThreadFingerprint struct {
	Print json.RawMessage `json:"print"`
	Code  *string         `json:"code"`
}
type CorpusComparison struct {
	Regressions []string `json:"regressions"`
	Changes     []string `json:"changes"`
}

func TraceCorpusLoad(source string, options CorpusSourceOptions) (json.RawMessage, error) {
	return CallOperation("trace.corpus.load", struct {
		Source string `json:"source"`
		CorpusSourceOptions
	}{source, options})
}
func TraceCorpusSource(source string, options CorpusSourceOptions) ([]Fingerprint, error) {
	return operationInto[[]Fingerprint]("trace.corpus.source", struct {
		Source string `json:"source"`
		CorpusSourceOptions
	}{source, options})
}
func TraceCorpusReachable(artifact json.RawMessage, options ReachableOptions) ([]Fingerprint, error) {
	return operationInto[[]Fingerprint]("trace.corpus.reachable", struct {
		Artifact json.RawMessage `json:"artifact"`
		ReachableOptions
	}{artifact, options})
}

// TraceCorpusWalkSeed explicitly reads a seed filename and may write branch
// output if Branches is selected. Other source operations take source text.
func TraceCorpusWalkSeed(seed string, options CorpusWalkOptions) (json.RawMessage, error) {
	return CallOperation("trace.corpus.walk_seed", struct {
		Seed string `json:"seed"`
		CorpusWalkOptions
	}{seed, options})
}

// TraceCorpusFingerprint replays the selected thread (portable PrefixTraceReport projection).
func TraceCorpusFingerprint(artifact json.RawMessage, options FingerprintOptions) (*ThreadFingerprint, error) {
	return operationInto[*ThreadFingerprint]("trace.corpus.fingerprint", struct {
		Artifact json.RawMessage `json:"artifact"`
		FingerprintOptions
	}{artifact, options})
}
func TraceCorpusCompare(baseline, current json.RawMessage) (*CorpusComparison, error) {
	return operationInto[*CorpusComparison]("trace.corpus.compare", struct {
		Baseline json.RawMessage `json:"baseline"`
		Current  json.RawMessage `json:"current"`
	}{baseline, current})
}
func TraceCorpusCompareInline(outlined, inline json.RawMessage) ([]string, error) {
	return operationInto[[]string]("trace.corpus.compare_inline", struct {
		Outlined json.RawMessage `json:"outlined"`
		Inline   json.RawMessage `json:"inline"`
	}{outlined, inline})
}

type NamedSource struct {
	Name   string `json:"name"`
	Source string `json:"source"`
}
type NamingAnalysis struct {
	Name   string          `json:"name"`
	Plan   json.RawMessage `json:"plan"`
	Rename json.RawMessage `json:"rename"`
}
type NamingDifference struct {
	Name            string   `json:"name"`
	BaselinePresent bool     `json:"baseline_present"`
	CurrentPresent  bool     `json:"current_present"`
	MissingRoles    []string `json:"missing_roles"`
	AddedRoles      []string `json:"added_roles"`
	NameDivergences []string `json:"name_divergences"`
}

func NamingCorpusAnalyze(sources []NamedSource) ([]NamingAnalysis, error) {
	return operationInto[[]NamingAnalysis]("naming.corpus.analyze", struct {
		Sources []NamedSource `json:"sources"`
	}{sources})
}

// NamingCorpusCompare reconstructs plans from explicit named program sources.
func NamingCorpusCompare(baseline, current []NamedSource) ([]NamingDifference, error) {
	return operationInto[[]NamingDifference]("naming.corpus.compare", struct {
		Baseline []NamedSource `json:"baseline"`
		Current  []NamedSource `json:"current"`
	}{baseline, current})
}

// OpsClassifyReportDocument classifies an explicit serialized PipelineReport,
// preserving source-backed report facts instead of reconstructing from raw text.
func OpsClassifyReportDocument(report json.RawMessage) (*Classification, error) {
	return operationInto[*Classification]("ops.classify.report", struct {
		Report json.RawMessage `json:"report"`
	}{report})
}

// OpsClassifyTemplateDocument uses a serialized HandlerTemplate and its handler
// source text. TemplateDocument and Template(index) select distinct inputs.
func OpsClassifyTemplateDocument(source string, template json.RawMessage) (*ClassifiedOp, error) {
	return operationInto[*ClassifiedOp]("ops.classify.template", struct {
		Source   string          `json:"source"`
		Template json.RawMessage `json:"template"`
	}{source, template})
}
