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
    if (argc != 3) {
        fprintf(stderr, "usage: %s artifact.json capture.json\n", argv[0]);
        return 2;
    }
    size_t length;
    char *bytes = read_file(argv[1], &length);
    char *error = NULL;
    ShapeArtifact *artifact = shape_artifact_load((const uint8_t *)bytes, length, &error);
    free(bytes);
    if (artifact == NULL) {
        fprintf(stderr, "%s\n", error ? error : "load failed");
        shape_free(error);
        return 1;
    }
    char *schema = value_of(shape_schema(artifact, "{}"));
    printf("schema: %s\n", schema);
    free(schema);
    char *capture = read_file(argv[2], NULL);
    char *request = request_field("capture", capture, NULL);
    free(capture);
    char *decoded = value_of(shape_decode(artifact, request));
    free(request);
    request = request_field("decoded", decoded, "\"signal\":\"DeviceMemory\"");
    char *detail = value_of(shape_explain(artifact, request));
    free(request);
    printf("DeviceMemory: %s\n", detail);
    free(detail);
    request = request_field("decoded", decoded, NULL);
    char *encoded = value_of(shape_encode(artifact, request));
    free(request);
    printf("unchanged headers: %s\n", encoded);
    free(encoded);

    /* Explicit named-signal edit, suitable for this synthetic fixture. For a
     * capture edit the full decoded document with a lossless JSON parser to
     * retain seeds, prefixes, sibling channels and metadata.originalReport. */
    encoded = value_of(shape_encode(artifact,
        "{\"signals\":{\"DeviceMemory\":{\"deviceMemory\":16}}}"));
    request = request_field("capture", encoded, NULL);
    char *edited = value_of(shape_decode(artifact, request));
    free(request);
    printf("edited payload: %s\n", edited);
    free(edited);
    free(encoded);
    free(decoded);
    shape_artifact_free(artifact);
    return 0;
}
