#include "shape.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *read_file(const char *path, size_t *length) {
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        perror(path);
        exit(1);
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        perror(path);
        exit(1);
    }
    long size = ftell(file);
    rewind(file);
    if (size < 0) {
        perror(path);
        exit(1);
    }
    char *data = malloc((size_t)size + 1);
    if (data == NULL || fread(data, 1, (size_t)size, file) != (size_t)size) {
        perror(path);
        exit(1);
    }
    fclose(file);
    data[size] = '\0';
    if (length != NULL) {
        *length = (size_t)size;
    }
    return data;
}

static char *request_field(const char *field, const char *json,
                           const char *extra) {
    size_t size = strlen(field) + strlen(json) + 8;
    if (extra != NULL) {
        size += strlen(extra) + 1;
    }
    char *text = malloc(size);
    if (text == NULL) {
        exit(1);
    }
    snprintf(text, size, "{\"%s\":%s%s%s}", field, json,
             extra == NULL ? "" : ",", extra == NULL ? "" : extra);
    return text;
}

/* Quote UTF-8 source as JSON without interpreting any numeric document tokens. */
static char *quote_json(const char *text) {
    size_t length = strlen(text);
    char *json = malloc(length * 6 + 3);
    if (json == NULL) exit(1);
    char *out = json;
    *out++ = '"';
    for (const unsigned char *p = (const unsigned char *)text; *p; ++p) {
        if (*p == '"' || *p == '\\') { *out++ = '\\'; *out++ = (char)*p; }
        else if (*p < 32) { snprintf(out, 7, "\\u%04x", *p); out += 6; }
        else *out++ = (char)*p;
    }
    *out++ = '"';
    *out = '\0';
    return json;
}

/* Copy a top-level JSON value as text. Input is valid library-produced JSON;
 * nested strings/containers are skipped without parsing integers as doubles. */
static char *field_json(const char *json, const char *key) {
    const char *p = json + 1;
    while (*p && *p != '}') {
        while (*p == ' ' || *p == '\n' || *p == ',' || *p == '\r' || *p == '\t') ++p;
        if (*p != '"') break;
        const char *name = ++p;
        while (*p && *p != '"') {
            if (*p == '\\' && p[1]) ++p;
            ++p;
        }
        size_t name_length = (size_t)(p - name);
        if (*p != '"') break;
        ++p;
        while (*p == ' ' || *p == ':') ++p;
        const char *start = p;
        int depth = 0, string = 0;
        for (; *p; ++p) {
            if (string) {
                if (*p == '\\' && p[1]) ++p;
                else if (*p == '"') string = 0;
            } else {
                if (depth == 0 && (*p == ',' || *p == '}')) break;
                if (*p == '"') string = 1;
                else if (*p == '{' || *p == '[') ++depth;
                else if (*p == '}' || *p == ']') --depth;
            }
        }
        if (strlen(key) == name_length && memcmp(name, key, name_length) == 0) {
            size_t length = (size_t)(p - start);
            char *value = malloc(length + 1);
            if (value == NULL) exit(1);
            memcpy(value, start, length);
            value[length] = '\0';
            return value;
        }
    }
    fprintf(stderr, "missing JSON field: %s\n", key);
    exit(1);
}

/* Free the library envelope after copying its value; propagate every error. */
static char *value_of(char *envelope) {
    if (envelope == NULL) { fprintf(stderr, "null response\n"); exit(1); }
    char *ok = field_json(envelope, "ok");
    if (strcmp(ok, "true") != 0) {
        fprintf(stderr, "shape call failed: %s\n", envelope);
        free(ok);
        shape_free(envelope);
        exit(1);
    }
    free(ok);
    char *value = field_json(envelope, "value");
    shape_free(envelope);
    return value;
}

int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr, "usage: %s seed.js init.js headers.json site-id\n", argv[0]);
        return 2;
    }
    char *raw_seed = read_file(argv[1], NULL);
    char *source_json = quote_json(raw_seed);
    free(raw_seed);
    char *trace_request = request_field("source", source_json, "\"cache\":{\"mode\":\"disabled\"}");
    free(source_json);
    char *traced = value_of(shape_call_v2("trace.program", trace_request));
    free(trace_request);
    char *complete = field_json(traced, "complete");
    if (strcmp(complete, "true") != 0) {
        fprintf(stderr, "incomplete trace: %s\n", traced);
        free(complete);
        free(traced);
        return 1;
    }
    free(complete);
    char *program_json = field_json(traced, "code"); /* JSON string literal */
    char *schema_request = request_field("source", program_json, NULL);
    char *signal_schema = value_of(shape_call_v2("signals.schema", schema_request));
    free(schema_request);
    printf("signal schema: %s\n", signal_schema);
    free(signal_schema);
    free(traced);
    char *initializer = read_file(argv[2], NULL);
    char *initializer_json = quote_json(initializer);
    free(initializer);
    char *session_request = request_field("init", initializer_json, NULL);
    free(initializer_json);
    char *session = value_of(shape_call_v2("session.from_initializer", session_request));
    free(session_request);
    char *site_id = quote_json(argv[4]);
    size_t extra_length = strlen(session) + strlen(site_id) + 32;
    char *extra = malloc(extra_length);
    if (extra == NULL) exit(1);
    snprintf(extra, extra_length, "\"session\":%s,\"site_id\":%s", session, site_id);
    free(session);
    free(site_id);
    char *compile_request = request_field("source", program_json, extra);
    free(program_json);
    free(extra);
    char *error = NULL;
    ShapeArtifact *artifact_handle = shape_artifact_compile(compile_request, &error);
    free(compile_request);
    if (artifact_handle == NULL) {
        fprintf(stderr, "compile failed: %s\n", error == NULL ? "(no message)" : error);
        shape_free(error);
        return 1;
    }

    /* Export deliberately for persistence; decoding already has a live handle. */
    char *artifact = value_of(shape_artifact_call(artifact_handle, "artifact", NULL));
    FILE *saved = fopen("artifact.json", "wb");
    if (saved == NULL) { perror("artifact.json"); return 1; }
    size_t artifact_length = strlen(artifact);
    if (fwrite(artifact, 1, artifact_length, saved) != artifact_length) return 1;
    if (fclose(saved) != 0) return 1;
    free(artifact);

    char *headers_json = read_file(argv[3], NULL);
    char *decode_request = request_field("capture", headers_json, NULL);
    free(headers_json);
    char *decoded = value_of(shape_decode(artifact_handle, decode_request));
    free(decode_request);

    char *explain_request =
        request_field("decoded", decoded, "\"signal\":\"DeviceMemory\"");
    char *signal = value_of(shape_explain(artifact_handle, explain_request));
    free(explain_request);
    printf("DeviceMemory: %s\n", signal);
    free(signal);

    /* Default decode retained originalReport, so full-payload encode reuses bytes. */
    char *encode_request = request_field("decoded", decoded, NULL);
    char *encoded = value_of(shape_encode(artifact_handle, encode_request));
    free(encode_request);
    printf("headers: %s\n", encoded);

    free(encoded);
    free(decoded);
    shape_artifact_free(artifact_handle);
    return 0;
}
