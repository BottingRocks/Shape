/*
 * addon.c: Node-API binding of libshape (sdk/c/include/shape.h).
 *
 * The native layer only moves JSON text across the boundary; index.js parses
 * envelopes and raises ShapeError. Exports:
 *
 *   callV2(command, requestJson)                -> envelope string
 *   callV2Async(command, requestJson)           -> Promise<envelope string>
 *   artifactCompileAsync(requestJson)          -> Promise<handle | envelope string>
 *   kernelLoad / kernelLoadAsync(bytes)        -> handle / Promise<handle>
 *   version()                                  -> string
 *   call(command, requestJson)                 -> envelope string
 *   callAsync(command, requestJson)            -> Promise<envelope string>
 *   generatorLoad(bytes)                       -> handle | envelope string
 *   generatorLoadAsync(bytes)                  -> Promise<handle | envelope string>
 *   generatorCall(handle, method, requestJson) -> envelope string
 *   generatorCallAsync(handle, method, json)   -> Promise<envelope string>
 *   generatorFree(handle)                      -> undefined
 *
 * Async calls run on the libuv thread pool (napi_create_async_work), so the
 * event loop keeps running while a trace takes seconds. A generator handle is
 * an external holding a reference-counted Holder: close() and the GC finalizer
 * mark it closed, and the ShapeGenerator is released once no async call still
 * uses it.
 */
#define NAPI_VERSION 8
#include <node_api.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "shape.h"

#define CHECK(env, call)                                                     \
    do {                                                                     \
        if ((call) != napi_ok) {                                             \
            throw_last(env);                                                 \
            return NULL;                                                     \
        }                                                                    \
    } while (0)

static const char CLOSED_ENVELOPE[] =
    "{\"ok\":false,\"error\":{\"code\":\"invalid_request\",\"message\":\"generator is closed\"}}";
static const char NULL_RESULT_ENVELOPE[] =
    "{\"ok\":false,\"error\":{\"code\":\"failed\",\"message\":\"libshape returned no result\"}}";

static void throw_last(napi_env env) {
    bool pending = false;
    napi_is_exception_pending(env, &pending);
    if (pending) return;
    const napi_extended_error_info *info = NULL;
    napi_get_last_error_info(env, &info);
    napi_throw_error(env, NULL,
                     info && info->error_message ? info->error_message : "Node-API call failed");
}

/* ---- generator holder ----------------------------------------------------- */

typedef struct {
    void *gen;
    int kernel;
    int pending;   /* async calls in flight (touched on the JS thread only) */
    int closed;    /* close() or finalizer ran: release gen when idle */
    int finalized; /* the external is gone: free the holder when idle */
} Holder;

static const napi_type_tag HOLDER_TAG = { UINT64_C(0xb0924345f65144d2), UINT64_C(0xa433f4de699a7c13) };

static void holder_settle(Holder *h) {
    if (h->pending > 0) return;
    if (h->closed && h->gen) {
        if (h->kernel) shape_kernel_free(h->gen);
        else shape_artifact_free(h->gen);
        h->gen = NULL;
    }
    if (h->finalized) free(h);
}

static void holder_finalize(napi_env env, void *data, void *hint) {
    (void)env;
    (void)hint;
    Holder *h = (Holder *)data;
    h->closed = 1;
    h->finalized = 1;
    holder_settle(h);
}

static void release_handle(void *gen, int kernel) {
    if (kernel) shape_kernel_free(gen);
    else shape_artifact_free(gen);
}

static napi_value make_handle(napi_env env, void *gen, int kernel) {
    Holder *h = (Holder *)calloc(1, sizeof(Holder));
    if (!h) {
        release_handle(gen, kernel);
        napi_throw_error(env, NULL, "out of memory");
        return NULL;
    }
    h->gen = gen;
    h->kernel = kernel;
    napi_value external;
    if (napi_create_external(env, h, holder_finalize, NULL, &external) != napi_ok) {
        release_handle(gen, kernel);
        free(h);
        throw_last(env);
        return NULL;
    }
    if (napi_type_tag_object(env, external, &HOLDER_TAG) != napi_ok) {
        /* The external's finalizer now owns h, even when tagging fails. */
        h->closed = 1;
        holder_settle(h);
        throw_last(env);
        return NULL;
    }
    return external;
}

static Holder *get_holder(napi_env env, napi_value value) {
    napi_valuetype type;
    if (napi_typeof(env, value, &type) != napi_ok || type != napi_external) {
        napi_throw_type_error(env, NULL, "expected a generator handle");
        return NULL;
    }
    bool tagged = false;
    if (napi_check_object_type_tag(env, value, &HOLDER_TAG, &tagged) != napi_ok || !tagged) {
        napi_throw_type_error(env, NULL, "invalid generator handle");
        return NULL;
    }
    void *data = NULL;
    if (napi_get_value_external(env, value, &data) != napi_ok || !data) {
        napi_throw_type_error(env, NULL, "invalid generator handle");
        return NULL;
    }
    return (Holder *)data;
}

/* ---- argument helpers ----------------------------------------------------- */

/* Copy a JS string into a malloc'd UTF-8 buffer. undefined/null -> *out = NULL
 * when optional. Returns 0 on success, -1 with an exception pending. */
static int get_string(napi_env env, napi_value value, int optional, const char *what, char **out) {
    *out = NULL;
    napi_valuetype type;
    if (napi_typeof(env, value, &type) != napi_ok) {
        throw_last(env);
        return -1;
    }
    if (optional && (type == napi_undefined || type == napi_null)) return 0;
    if (type != napi_string) {
        char message[96];
        snprintf(message, sizeof message, "%s must be a string", what);
        napi_throw_type_error(env, NULL, message);
        return -1;
    }
    size_t len = 0;
    if (napi_get_value_string_utf8(env, value, NULL, 0, &len) != napi_ok) {
        throw_last(env);
        return -1;
    }
    char *buf = (char *)malloc(len + 1);
    if (!buf) {
        napi_throw_error(env, NULL, "out of memory");
        return -1;
    }
    if (napi_get_value_string_utf8(env, value, buf, len + 1, &len) != napi_ok) {
        free(buf);
        throw_last(env);
        return -1;
    }
    // C strings must never silently truncate a command or raw JSON document.
    if (memchr(buf, '\0', len)) {
        free(buf);
        char message[96];
        snprintf(message, sizeof message, "%s must not contain NUL bytes", what);
        napi_throw_type_error(env, NULL, message);
        return -1;
    }
    *out = buf;
    return 0;
}

/* Copy a Buffer / Uint8Array / ArrayBuffer into a malloc'd byte buffer. */
static int get_bytes(napi_env env, napi_value value, uint8_t **out, size_t *out_len) {
    *out = NULL;
    *out_len = 0;
    void *data = NULL;
    size_t len = 0;
    bool is = false;
    if (napi_is_typedarray(env, value, &is) == napi_ok && is) {
        napi_typedarray_type type;
        size_t length = 0, offset = 0;
        napi_value arraybuffer;
        if (napi_get_typedarray_info(env, value, &type, &length, &data, &arraybuffer, &offset) != napi_ok) {
            throw_last(env);
            return -1;
        }
        if (type != napi_uint8_array && type != napi_uint8_clamped_array && type != napi_int8_array) {
            napi_throw_type_error(env, NULL, "artifact bytes must be a Uint8Array or Buffer");
            return -1;
        }
        len = length;
    } else if (napi_is_arraybuffer(env, value, &is) == napi_ok && is) {
        if (napi_get_arraybuffer_info(env, value, &data, &len) != napi_ok) {
            throw_last(env);
            return -1;
        }
    } else {
        napi_throw_type_error(env, NULL, "artifact bytes must be a Buffer, Uint8Array or ArrayBuffer");
        return -1;
    }
    uint8_t *copy = (uint8_t *)malloc(len ? len : 1);
    if (!copy) {
        napi_throw_error(env, NULL, "out of memory");
        return -1;
    }
    if (len) memcpy(copy, data, len);
    *out = copy;
    *out_len = len;
    return 0;
}

/* Turn a libshape result into a JS string and free it (NULL -> fallback). */
static napi_value take_result(napi_env env, char *result) {
    napi_value out;
    napi_status status = napi_create_string_utf8(
        env, result ? result : NULL_RESULT_ENVELOPE, NAPI_AUTO_LENGTH, &out);
    shape_free(result);
    if (status != napi_ok) {
        throw_last(env);
        return NULL;
    }
    return out;
}

static napi_value make_string(napi_env env, const char *text) {
    napi_value out;
    CHECK(env, napi_create_string_utf8(env, text, NAPI_AUTO_LENGTH, &out));
    return out;
}

/* ---- sync exports ------------------------------------------------------------ */

static napi_value js_version(napi_env env, napi_callback_info info) {
    (void)info;
    return make_string(env, shape_version());
}

static napi_value js_call(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value argv[2];
    void *mode = NULL;
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, &mode));
    if (argc < 1) {
        napi_throw_type_error(env, NULL, "call(command, requestJson)");
        return NULL;
    }
    char *command = NULL, *request = NULL;
    if (get_string(env, argv[0], 0, "command", &command)) return NULL;
    if (argc > 1 && get_string(env, argv[1], 1, "request", &request)) {
        free(command);
        return NULL;
    }
    char *result = mode ? shape_call_v2(command, request) : shape_call(command, request);
    free(command);
    free(request);
    return take_result(env, result);
}

static napi_value js_generator_load(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value argv[1];
    void *mode = NULL;
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, &mode));
    if (argc < 1) {
        napi_throw_type_error(env, NULL, "generatorLoad(bytes)");
        return NULL;
    }
    uint8_t *bytes = NULL;
    size_t len = 0;
    if (get_bytes(env, argv[0], &bytes, &len)) return NULL;
    char *error = NULL;
    void *gen = mode ? (void *)shape_kernel_load(bytes, len, &error)
                     : (void *)shape_artifact_load(bytes, len, &error);
    free(bytes);
    if (!gen) return take_result(env, error);
    shape_free(error);
    return make_handle(env, gen, mode != NULL);
}

static napi_value js_generator_call(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value argv[3];
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, NULL));
    if (argc < 2) {
        napi_throw_type_error(env, NULL, "generatorCall(handle, method, requestJson)");
        return NULL;
    }
    Holder *h = get_holder(env, argv[0]);
    if (!h) return NULL;
    char *method = NULL, *request = NULL;
    if (get_string(env, argv[1], 0, "method", &method)) return NULL;
    if (argc > 2 && get_string(env, argv[2], 1, "request", &request)) {
        free(method);
        return NULL;
    }
    if (h->closed || !h->gen) {
        free(method);
        free(request);
        return make_string(env, CLOSED_ENVELOPE);
    }
    char *result = h->kernel ? shape_kernel_call(h->gen, method, request)
                             : shape_artifact_call(h->gen, method, request);
    free(method);
    free(request);
    return take_result(env, result);
}

static napi_value js_generator_free(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value argv[1];
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, NULL));
    if (argc < 1) {
        napi_throw_type_error(env, NULL, "generatorFree(handle)");
        return NULL;
    }
    Holder *h = get_holder(env, argv[0]);
    if (!h) return NULL;
    h->closed = 1;
    holder_settle(h);
    return NULL;
}

/* ---- async work ----------------------------------------------------------------- */

typedef enum { WORK_CALL, WORK_LOAD, WORK_COMPILE, WORK_GENERATOR } WorkKind;

typedef struct {
    WorkKind kind;
    int mode;
    napi_async_work work;
    napi_deferred deferred;
    char *name;        /* command or method */
    char *request;     /* request JSON (may be NULL) */
    uint8_t *bytes;    /* artifact bytes for WORK_LOAD */
    size_t len;
    Holder *holder;    /* WORK_GENERATOR */
    void *gen; /* WORK_LOAD result */
    char *result;      /* envelope (owned by libshape) */
} Work;

static void work_free(Work *w) {
    free(w->name);
    free(w->request);
    free(w->bytes);
    shape_free(w->result);
    if (w->gen) release_handle(w->gen, w->mode);
    free(w);
}

static void work_execute(napi_env env, void *data) {
    (void)env;
    Work *w = (Work *)data;
    switch (w->kind) {
    case WORK_CALL:
        w->result = w->mode ? shape_call_v2(w->name, w->request) : shape_call(w->name, w->request);
        break;
    case WORK_LOAD:
        w->gen = w->mode ? (void *)shape_kernel_load(w->bytes, w->len, &w->result)
                         : (void *)shape_artifact_load(w->bytes, w->len, &w->result);
        break;
    case WORK_COMPILE:
        w->gen = shape_artifact_compile(w->request, &w->result);
        break;
    case WORK_GENERATOR:
        /* The holder keeps gen alive while pending > 0. */
        w->result = w->holder->kernel
            ? shape_kernel_call(w->holder->gen, w->name, w->request)
            : shape_artifact_call(w->holder->gen, w->name, w->request);
        break;
    }
}

static void work_complete(napi_env env, napi_status status, void *data) {
    Work *w = (Work *)data;
    napi_value value = NULL;
    int reject = 0;

    if (w->kind == WORK_GENERATOR) {
        w->holder->pending--;
    }

    if (status != napi_ok) {
        napi_value message;
        napi_create_string_utf8(env, status == napi_cancelled ? "shape call cancelled" : "shape call failed",
                                NAPI_AUTO_LENGTH, &message);
        napi_create_error(env, NULL, message, &value);
        reject = 1;
    } else if ((w->kind == WORK_LOAD || w->kind == WORK_COMPILE) && w->gen) {
        void *gen = w->gen;
        w->gen = NULL; /* owned by the handle now (or freed by make_handle on failure) */
        value = make_handle(env, gen, w->mode);
        if (!value) {
            bool pending = false;
            napi_is_exception_pending(env, &pending);
            if (pending) napi_get_and_clear_last_exception(env, &value);
            reject = 1;
        }
    } else {
        const char *text = w->result ? w->result
                         : (w->kind == WORK_GENERATOR && !w->holder->gen) ? CLOSED_ENVELOPE
                         : NULL_RESULT_ENVELOPE;
        if (napi_create_string_utf8(env, text, NAPI_AUTO_LENGTH, &value) != napi_ok) {
            napi_value message;
            napi_create_string_utf8(env, "result too large for a JS string", NAPI_AUTO_LENGTH, &message);
            napi_create_error(env, NULL, message, &value);
            reject = 1;
        }
    }

    if (reject)
        napi_reject_deferred(env, w->deferred, value);
    else
        napi_resolve_deferred(env, w->deferred, value);

    napi_delete_async_work(env, w->work);
    if (w->kind == WORK_GENERATOR) holder_settle(w->holder);
    work_free(w);
}

/* Queue w; on failure free it and return NULL with an exception pending. */
static napi_value queue(napi_env env, Work *w, const char *resource) {
    napi_value promise, name;
    if (napi_create_promise(env, &w->deferred, &promise) != napi_ok ||
        napi_create_string_utf8(env, resource, NAPI_AUTO_LENGTH, &name) != napi_ok ||
        napi_create_async_work(env, NULL, name, work_execute, work_complete, w, &w->work) != napi_ok) {
        throw_last(env);
        work_free(w);
        return NULL;
    }
    if (napi_queue_async_work(env, w->work) != napi_ok) {
        throw_last(env);
        napi_delete_async_work(env, w->work);
        work_free(w);
        return NULL;
    }
    if (w->kind == WORK_GENERATOR) w->holder->pending++;
    return promise;
}

static Work *work_new(napi_env env, WorkKind kind) {
    Work *w = (Work *)calloc(1, sizeof(Work));
    if (!w) napi_throw_error(env, NULL, "out of memory");
    else w->kind = kind;
    return w;
}

static napi_value js_call_async(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value argv[2];
    void *mode = NULL;
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, &mode));
    if (argc < 1) {
        napi_throw_type_error(env, NULL, "callAsync(command, requestJson)");
        return NULL;
    }
    Work *w = work_new(env, WORK_CALL);
    if (!w) return NULL;
    w->mode = mode != NULL;
    if (get_string(env, argv[0], 0, "command", &w->name) ||
        (argc > 1 && get_string(env, argv[1], 1, "request", &w->request))) {
        work_free(w);
        return NULL;
    }
    return queue(env, w, "shape.call");
}

static napi_value js_generator_load_async(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value argv[1];
    void *mode = NULL;
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, &mode));
    if (argc < 1) {
        napi_throw_type_error(env, NULL, "generatorLoadAsync(bytes)");
        return NULL;
    }
    Work *w = work_new(env, WORK_LOAD);
    if (!w) return NULL;
    w->mode = mode != NULL;
    if (get_bytes(env, argv[0], &w->bytes, &w->len)) {
        work_free(w);
        return NULL;
    }
    return queue(env, w, "shape.generatorLoad");
}

static napi_value js_generator_call_async(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value argv[3];
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, NULL));
    if (argc < 2) {
        napi_throw_type_error(env, NULL, "generatorCallAsync(handle, method, requestJson)");
        return NULL;
    }
    Holder *h = get_holder(env, argv[0]);
    if (!h) return NULL;
    Work *w = work_new(env, WORK_GENERATOR);
    if (!w) return NULL;
    if (get_string(env, argv[1], 0, "method", &w->name) ||
        (argc > 2 && get_string(env, argv[2], 1, "request", &w->request))) {
        work_free(w);
        return NULL;
    }
    if (h->closed || !h->gen) {
        /* Settle immediately with the closed envelope. */
        napi_deferred deferred;
        napi_value promise, value;
        work_free(w);
        CHECK(env, napi_create_promise(env, &deferred, &promise));
        CHECK(env, napi_create_string_utf8(env, CLOSED_ENVELOPE, NAPI_AUTO_LENGTH, &value));
        CHECK(env, napi_resolve_deferred(env, deferred, value));
        return promise;
    }
    w->holder = h;
    return queue(env, w, "shape.generatorCall");
}

/* Compile directly into the validated native state; no artifact JSON export. */
static napi_value js_artifact_compile_async(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value argv[1];
    CHECK(env, napi_get_cb_info(env, info, &argc, argv, NULL, NULL));
    if (argc < 1) {
        napi_throw_type_error(env, NULL, "artifactCompileAsync(requestJson)");
        return NULL;
    }
    Work *w = work_new(env, WORK_COMPILE);
    if (!w) return NULL;
    if (get_string(env, argv[0], 0, "request", &w->request)) {
        work_free(w);
        return NULL;
    }
    return queue(env, w, "shape.artifactCompile");
}

/* ---- module ------------------------------------------------------------------------ */

NAPI_MODULE_INIT() {
    napi_property_descriptor props[] = {
        {"artifactCompileAsync", NULL, js_artifact_compile_async, NULL, NULL, NULL, napi_enumerable, NULL},
        {"callV2", NULL, js_call, NULL, NULL, NULL, napi_enumerable, (void *)1},
        {"callV2Async", NULL, js_call_async, NULL, NULL, NULL, napi_enumerable, (void *)1},
        {"kernelLoad", NULL, js_generator_load, NULL, NULL, NULL, napi_enumerable, (void *)1},
        {"kernelLoadAsync", NULL, js_generator_load_async, NULL, NULL, NULL, napi_enumerable, (void *)1},
        {"version", NULL, js_version, NULL, NULL, NULL, napi_enumerable, NULL},
        {"call", NULL, js_call, NULL, NULL, NULL, napi_enumerable, NULL},
        {"callAsync", NULL, js_call_async, NULL, NULL, NULL, napi_enumerable, NULL},
        {"generatorLoad", NULL, js_generator_load, NULL, NULL, NULL, napi_enumerable, NULL},
        {"generatorLoadAsync", NULL, js_generator_load_async, NULL, NULL, NULL, napi_enumerable, NULL},
        {"generatorCall", NULL, js_generator_call, NULL, NULL, NULL, napi_enumerable, NULL},
        {"generatorCallAsync", NULL, js_generator_call_async, NULL, NULL, NULL, napi_enumerable, NULL},
        {"generatorFree", NULL, js_generator_free, NULL, NULL, NULL, napi_enumerable, NULL},
    };
    if (napi_define_properties(env, exports, sizeof props / sizeof props[0], props) != napi_ok) {
        throw_last(env);
        return NULL;
    }
    return exports;
}
