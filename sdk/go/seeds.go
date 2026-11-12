package shape

// Seed acquisition is explicit. Parsing and URL operations never fetch; only
// SeedsFetch/SeedsFetchDirect use the network. SeedsImport is explicit storage.
type Launcher struct {
	KernelRef string `json:"kernel_ref"`
}
type KernelSummary struct {
	Bytes      uint64 `json:"bytes"`
	SHA256     string `json:"sha256"`
	Statements uint64 `json:"statements"`
	Functions  uint64 `json:"functions"`
}
type SeedScript struct {
	Kind     string         `json:"kind"`
	Launcher *Launcher      `json:"launcher,omitempty"`
	Kernel   *KernelSummary `json:"kernel,omitempty"`
}
type Site struct {
	Name        string      `json:"name"`
	LauncherURL string      `json:"launcher_url"`
	BaseURL     *string     `json:"base_url"`
	SeedParam   string      `json:"seed_param"`
	Protocol    string      `json:"protocol"`
	Status      string      `json:"status"`
	Headers     [][2]string `json:"headers"`
}
type ClientConfig struct {
	TimeoutMS        *uint64 `json:"timeout_ms,omitempty"`
	ConnectTimeoutMS *uint64 `json:"connect_timeout_ms,omitempty"`
	MaxBodyBytes     *uint64 `json:"max_body_bytes,omitempty"`
	MaxRedirects     *int    `json:"max_redirects,omitempty"`
}
type FetchOptions struct {
	Client    *ClientConfig `json:"client,omitempty"`
	Headers   [][2]string   `json:"headers,omitempty"`
	BaseURL   string        `json:"base_url,omitempty"`
	SeedParam string        `json:"seed_param,omitempty"`
}
type FetchedSeed struct {
	Seed          string        `json:"seed"`
	LauncherURL   string        `json:"launcher_url"`
	KernelURL     string        `json:"kernel_url"`
	Launcher      Bytes         `json:"launcher"`
	Kernel        Bytes         `json:"kernel"`
	KernelSummary KernelSummary `json:"kernel_summary"`
}
type StoreOutcome struct {
	Status          string  `json:"status"`
	KernelPath      string  `json:"kernel_path"`
	LauncherPath    *string `json:"launcher_path"`
	LauncherDiffers bool    `json:"launcher_differs"`
}

func SeedsParse(bytes []byte) (*SeedScript, error) {
	return operationInto[*SeedScript]("seeds.parse", bytesRequest{Bytes(bytes)})
}
func SeedsParseLauncher(bytes []byte) (*Launcher, error) {
	return operationInto[*Launcher]("seeds.parse_launcher", bytesRequest{Bytes(bytes)})
}
func SeedsParseKernel(bytes []byte) (*KernelSummary, error) {
	return operationInto[*KernelSummary]("seeds.parse_kernel", bytesRequest{Bytes(bytes)})
}
func SeedsParseURL(input string) (string, error) {
	return operationInto[string]("seeds.parse_url", struct {
		Input string `json:"input"`
	}{input})
}
func SeedsResolveURL(reference, base string) (string, error) {
	return operationInto[string]("seeds.resolve_url", struct {
		Reference string `json:"reference"`
		Base      string `json:"base,omitempty"`
	}{reference, base})
}
func SeedsToken(reference, param string) (string, error) {
	return operationInto[string]("seeds.token", struct {
		Reference string `json:"reference"`
		Param     string `json:"param,omitempty"`
	}{reference, param})
}
func SeedsSites() ([]Site, error) { return operationInto[[]Site]("seeds.sites", nil) }
func SeedsSite(name string) (*Site, error) {
	return operationInto[*Site]("seeds.site", struct {
		Name string `json:"name"`
	}{name})
}
func SeedsClient(config ClientConfig) (*ClientConfig, error) {
	return operationInto[*ClientConfig]("seeds.client", config)
}
func SeedsFetch(url string, options FetchOptions) (*FetchedSeed, error) {
	return operationInto[*FetchedSeed]("seeds.fetch", struct {
		URL string `json:"url"`
		FetchOptions
	}{url, options})
}
func SeedsFetchDirect(url string, options FetchOptions) (*FetchedSeed, error) {
	return operationInto[*FetchedSeed]("seeds.fetch_direct", struct {
		URL string `json:"url"`
		FetchOptions
	}{url, options})
}
func SeedsImport(directory, seed string, launcher, kernel []byte) (*StoreOutcome, error) {
	var launcherBytes *Bytes
	if launcher != nil {
		value := Bytes(launcher)
		launcherBytes = &value
	}
	return operationInto[*StoreOutcome]("seeds.import", struct {
		Directory string `json:"directory"`
		Seed      string `json:"seed"`
		Launcher  *Bytes `json:"launcher,omitempty"`
		Kernel    Bytes  `json:"kernel"`
	}{directory, seed, launcherBytes, Bytes(kernel)})
}
