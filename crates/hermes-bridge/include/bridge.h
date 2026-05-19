#ifndef HERMES_BRIDGE_H
#define HERMES_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct HermesRuntime HermesRuntime;

// Create a new Hermes runtime. Returns NULL on failure.
HermesRuntime* hermes_create_runtime(void);

// Evaluate a JS string. Returns a newly allocated string (caller must free with
// hermes_free_string) containing the result of JSON.stringify on the return
// value, or NULL if evaluation failed.
// On error, *error_out is set to an allocated error message (caller frees).
char* hermes_eval(
    HermesRuntime* rt,
    const char* source,
    size_t source_len,
    const char* source_url,
    char** error_out);

// Free a string returned by hermes_eval.
void hermes_free_string(char* s);

// Destroy a runtime.
void hermes_destroy_runtime(HermesRuntime* rt);

#ifdef __cplusplus
}
#endif

#endif // HERMES_BRIDGE_H
