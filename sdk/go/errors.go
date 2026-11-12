package shape

// Error codes the library returns.
const (
	// CodeInvalidRequest: the request is not JSON, misses a field or has a
	// field of the wrong type.
	CodeInvalidRequest = "invalid_request"
	// CodeUnknownCommand: no such command or generator method.
	CodeUnknownCommand = "unknown_command"
	// CodeFailed: the request was valid but the operation failed.
	CodeFailed = "failed"
	// CodePanic: the library hit an internal bug; the process is unharmed.
	CodePanic = "panic"
)

// Error is every failure this package returns, from the library's error
// envelope or from the binding itself. Inspect it with errors.As:
//
//	var serr *shape.Error
//	if errors.As(err, &serr) && serr.Code == shape.CodeInvalidRequest { … }
type Error struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func (e *Error) Error() string {
	return "shape: " + e.Code + ": " + e.Message
}
