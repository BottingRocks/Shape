/* Compile and run unchanged as both C99 and C++11. */
#include "shape.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void check(char *result, const char *expected) {
    assert(result != NULL);
    assert(strstr(result, expected) != NULL);
    shape_free(result);
}

int main(int argc, char **argv) {
    assert(argc == 2);
    const char *version = shape_version();
    assert(version != NULL && strlen(version) > 0);
    assert(version == shape_version());
    check(shape_call("version", NULL), "\"ok\":true");
    check(shape_call_v2("version", NULL), "\"contract_version\":2");
    check(shape_call_v2(NULL, NULL), "invalid_request");
    {
        char *compile_error = NULL;
        assert(shape_artifact_compile(NULL, &compile_error) == NULL);
        check(compile_error, "invalid_request");
    }
    check(shape_decode(NULL, NULL), "invalid_request");
    check(shape_encode(NULL, NULL), "invalid_request");
    check(shape_explain(NULL, NULL), "invalid_request");
    check(shape_schema(NULL, NULL), "invalid_request");
    shape_artifact_free(NULL);
    shape_kernel_free(NULL);
    check(shape_call(NULL, "{}"), "invalid_request");
    check(shape_call("version", "[]"), "invalid_request");

    FILE *input = fopen(argv[1], "rb");
    assert(input != NULL);
    assert(fseek(input, 0, SEEK_END) == 0);
    long size = ftell(input);
    assert(size > 0);
    rewind(input);
    uint8_t *bytes = (uint8_t *)malloc((size_t)size);
    assert(bytes != NULL);
    assert(fread(bytes, 1, (size_t)size, input) == (size_t)size);
    assert(fclose(input) == 0);

    char *previous = shape_call(NULL, NULL);
    char *error = previous;
    ShapeGenerator *generator = shape_generator_load(bytes, (size_t)size, &error);
    assert(generator != NULL && error == NULL);
    check(previous, "invalid_request");
    memset(bytes, 0xff, (size_t)size);
    free(bytes);
    char *schema = shape_generator_call(generator, "schema", NULL);
    check(shape_schema(generator, NULL), "\"ok\":true");
    check(shape_artifact_call(generator, "artifact", NULL), "\"ok\":true");
    check(shape_generator_call(generator, "unknown", "{}"), "unknown_command");
    shape_generator_free(generator);
    check(schema, "\"ok\":true");

    error = NULL;
    assert(shape_generator_load(NULL, 0, &error) == NULL);
    check(error, "invalid_request");
    check(shape_generator_call(NULL, "schema", "{}"), "invalid_request");
    shape_generator_free(NULL);
    shape_free(NULL);
    puts("C/C++ ABI consumer passed");
    return 0;
}
