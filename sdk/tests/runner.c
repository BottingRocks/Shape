/* No JSON reserialization: preserve every integer digit from libshape. */
#include "shape.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *read_file(const char *path, size_t *length) {
    FILE *file = fopen(path, "rb");
    if (!file || fseek(file, 0, SEEK_END)) { perror(path); exit(2); }
    long size = ftell(file);
    if (size < 0 || fseek(file, 0, SEEK_SET)) { perror(path); exit(2); }
    char *data = malloc((size_t)size + 1);
    if (!data || fread(data, 1, (size_t)size, file) != (size_t)size) exit(2);
    fclose(file);
    data[size] = 0;
    *length = (size_t)size;
    return data;
}
int main(int argc, char **argv) {
    if (argc != 5) return 2;
    size_t length;
    char *request = read_file(argv[4], &length);
    ShapeGenerator *generator = NULL;
    char *result = NULL;
    if (!strcmp(argv[2], "kernel")) {
        char *artifact = read_file(argv[1], &length);
        ShapeKernel *kernel = shape_kernel_load((const uint8_t *)artifact, length, &result);
        free(artifact);
        if (kernel) {
            result = shape_kernel_call(kernel, argv[3], request);
            shape_kernel_free(kernel);
        }
    } else if (!strcmp(argv[2], "loaded")) {
        char *artifact = read_file(argv[1], &length);
        ShapeArtifact *loaded = shape_artifact_load((const uint8_t *)artifact, length, &result);
        free(artifact);
        if (loaded) {
            char *detached = shape_artifact_call(loaded, "artifact", "{}");
            if (!detached) { shape_artifact_free(loaded); free(request); return 2; }
            detached[0] = '!'; /* Export buffers are independent of loaded state. */
            shape_free(detached);
            char *first = NULL;
            int rounds = strcmp(argv[3], "encode") ? 3 : 1;
            for (int i = 0; i < rounds; i++) {
                if (!strcmp(argv[3], "decode")) result = shape_decode(loaded, request);
                else if (!strcmp(argv[3], "encode")) result = shape_encode(loaded, request);
                else if (!strcmp(argv[3], "explain")) result = shape_explain(loaded, request);
                else if (!strcmp(argv[3], "schema")) result = shape_schema(loaded, request);
                else result = shape_artifact_call(loaded, argv[3], request);
                if (!result) { shape_artifact_free(loaded); free(request); return 2; }
                if (!i) first = result;
                else if (strcmp(first, result)) { fputs("loaded operation changed on reuse\n", stderr); exit(2); }
                if (i == 1) shape_free(result);
            }
            if (rounds > 1) shape_free(first);
            shape_artifact_free(loaded);
        }
    } else if (!strcmp(argv[2], "named") && !strcmp(argv[3], "artifact.compile")) {
        ShapeArtifact *loaded = shape_artifact_compile(request, &result);
        if (loaded) {
            result = shape_artifact_call(loaded, "artifact", "{}");
            shape_artifact_free(loaded);
        }
    } else if (!strcmp(argv[2], "named") || !strcmp(argv[2], "v2")) {
        result = shape_call_v2(argv[3], request);
    } else if (!strcmp(argv[2], "generator")) {
        char *artifact = read_file(argv[1], &length);
        generator = shape_generator_load((const uint8_t *)artifact, length, &result);
        free(artifact);
        if (generator) result = shape_generator_call(generator, argv[3], request);
    } else {
        result = shape_call(argv[3], request);
    }
    free(request);
    if (!result) { shape_generator_free(generator); return 2; }
    puts(result);
    shape_free(result);
    shape_generator_free(generator);
    return 0;
}
