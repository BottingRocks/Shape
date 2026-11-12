/*
 * shape.h: C ABI of the shape SDK (libshape).
 *
 * Command requests and results are NUL-terminated UTF-8 JSON. Artifact loads
 * instead take an explicit byte length. Every command result is an envelope:
 *
 *   {"ok":true,"value":<result>}
 *   {"ok":false,"error":{"code":"invalid_request|unknown_command|failed|panic","message":"..."}}
 *
 * Strings returned by this library are owned by the caller and released with
 * shape_free(), except for the static shape_version() string. Invalid JSON
 * and required NULL arguments produce errors; other input pointers must be valid as
 * described below. Unwinding Rust panics during calls and loads become panic
 * envelopes. Process aborts (including allocation failure), invalid pointers,
 * double frees and data races cannot be recovered. Calls may run for
 * seconds (trace.program); they block the calling thread and are safe to make
 * from several threads at once. See sdk/c/README.md for every command.
 *
 * Numbers: decode artifacts and decoded payloads contain JSON integers above
 * 2^53 (for example f64 bit patterns and u64 superpack values). They must
 * round-trip exactly, so callers must not route them through a C double: a
 * double's 53-bit mantissa silently changes them. Parse and print such JSON
 * integers exactly (or pass the JSON text through unchanged). The library
 * itself only exchanges JSON text, so exactness is the caller's serialization
 * choice. artifact.compile defaults "float_byte_order" to "little" when the
 * field is omitted. Its optional "direct" field must be a JSON boolean
 * (default false); true opts into source-format decoding without lifecycle
 * certification. Saved artifacts retain that provenance on load.
 */
#ifndef SHAPE_H
#define SHAPE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* A loaded, validated decode artifact. */
typedef struct ShapeGenerator ShapeGenerator;
/* Named decode artifact handle; ABI-identical to the compatibility generator. */
typedef ShapeGenerator ShapeArtifact;
typedef struct ShapeKernel ShapeKernel;

/* Contract v2: sdk/operations.json. Explicit cache defaults to disabled;
 * trace.program defaults thread_cleanup=true,no_cleanup=false,jobs=null.
 * Same pointer, ownership and error-envelope rules as shape_call. */
char *shape_call_v2(const char *operation, const char *request_json);

/* Immutable artifacts parse/validate once. Methods: decode, encode, explain,
 * schema, artifact (exports the executable document for caller persistence).
 * Share across concurrent calls; wait
 * for all calls before freeing. Byte-load rules match shape_generator_load. */
ShapeArtifact *shape_artifact_load(const uint8_t *json, size_t len, char **error_out);
/* Same request as v2 artifact.compile; NULL/empty means {}. Returns a fully
 * compiled/validated owned handle without document serialization or reload.
 * On failure returns NULL and optionally stores an owned error envelope.
 * On success clears *error_out. Previous slot contents remain caller-owned.
 * String rules match shape_call; error_out rules match shape_generator_load.
 * Export deliberately with shape_artifact_call(handle,"artifact",NULL). */
ShapeArtifact *shape_artifact_compile(const char *request_json, char **error_out);
char *shape_artifact_call(const ShapeArtifact *artifact, const char *method, const char *request_json);
void shape_artifact_free(ShapeArtifact *artifact);
char *shape_decode(const ShapeArtifact *artifact, const char *request_json);
char *shape_encode(const ShapeArtifact *artifact, const char *request_json);
char *shape_explain(const ShapeArtifact *artifact, const char *request_json);
char *shape_schema(const ShapeArtifact *artifact, const char *request_json);

/* Operations: trace.program (requires matching raw source), trace.thread,
 * trace.corpus.reachable, kernel.to_json. Kernel reuse never reads disk cache.
 * Method request/result shapes are the same as v2 dispatch. */
ShapeKernel *shape_kernel_load(const uint8_t *json, size_t len, char **error_out);
char *shape_kernel_call(const ShapeKernel *kernel, const char *operation, const char *request_json);
void shape_kernel_free(ShapeKernel *kernel);

/* The SDK version, e.g. "0.1.0". Static: do not free. */
const char *shape_version(void);

/* Run a stateless command, e.g. shape_call("deob", "{\"source\":\"...\"}").
 * command must point to a NUL-terminated UTF-8 string; NULL produces an error.
 * request_json may be NULL or "" for {}; otherwise it must point to a
 * NUL-terminated UTF-8 JSON object. Invalid UTF-8 produces an error.
 * Only bytes before the first NUL are read; use JSON escapes for embedded NULs.
 * Each string, including its terminator, must fit within PTRDIFF_MAX bytes in
 * one allocation. All non-NULL pointers must meet their C type's alignment.
 * Inputs are borrowed for this call only and must remain readable and unchanged
 * until it returns. Free the result with shape_free(). */
char *shape_call(const char *command, const char *request_json);

/* Load a decode artifact (len bytes of JSON). Returns NULL on failure and
 * stores an error envelope in *error_out when error_out is not NULL (free with
 * shape_free()). On success *error_out is set to NULL.
 * artifact_json must point to len readable bytes within a single allocation;
 * NULL and lengths greater than PTRDIFF_MAX produce an error. The library
 * copies the bytes during this call; the caller may release them on return.
 * No NUL terminator is required; all len bytes must be UTF-8 JSON, including
 * any embedded NUL bytes (which are invalid JSON unless escaped).
 * The input must remain readable and unchanged until this call returns.
 * error_out must be NULL or writable and must not overlap the artifact bytes.
 * Callers retain ownership of any previous value of *error_out. */
ShapeGenerator *shape_generator_load(const uint8_t *artifact_json, size_t len, char **error_out);

/* method: "decode", "explain", "encode" or "schema". String arguments follow
 * shape_call's rules. generator must be NULL (an error) or a live handle from
 * shape_generator_load. Concurrent calls may share a live handle.
 * Every calling thread must keep the handle alive until its call returns.
 * Free the result with shape_free(). Results remain valid after handle release. */
char *shape_generator_call(const ShapeGenerator *generator, const char *method, const char *request_json);

/* Release a generator exactly once (NULL is ignored). Wait for all calls using
 * this handle to finish before freeing it; never use the handle afterwards. */
void shape_generator_free(ShapeGenerator *generator);

/* Release an owned result string exactly once (NULL is ignored).
 * Pass the original pointer from this same loaded library, without modifying
 * the string or its terminator. Do not use C free()/delete on library results.
 * Never pass the static shape_version() pointer to this function. */
void shape_free(char *text);

#ifdef __cplusplus
}
#endif

#endif /* SHAPE_H */
