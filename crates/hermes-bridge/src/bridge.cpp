#include "bridge.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>

namespace jsi = facebook::jsi;

struct HermesRuntime {
  std::unique_ptr<facebook::hermes::HermesRuntime> rt;
};

static char* strdup_alloc(const std::string& s) {
  char* out = static_cast<char*>(malloc(s.size() + 1));
  if (out) {
    memcpy(out, s.data(), s.size());
    out[s.size()] = '\0';
  }
  return out;
}

extern "C" {

HermesRuntime* hermes_create_runtime(void) {
  try {
    auto wrapper = new HermesRuntime();
    wrapper->rt = facebook::hermes::makeHermesRuntime();
    return wrapper;
  } catch (...) {
    return nullptr;
  }
}

char* hermes_eval(
    HermesRuntime* rt,
    const char* source,
    size_t source_len,
    const char* source_url,
    char** error_out) {
  if (error_out)
    *error_out = nullptr;
  try {
    auto& runtime = *rt->rt;
    auto result = runtime.evaluateJavaScript(
        std::make_shared<jsi::StringBuffer>(
            std::string(source, source_len)),
        source_url ? source_url : "eval");

    // Call JSON.stringify on the result
    auto json = runtime.global().getPropertyAsObject(runtime, "JSON");
    auto stringify = json.getPropertyAsFunction(runtime, "stringify");
    auto jsonStr = stringify.call(runtime, result);

    if (jsonStr.isUndefined()) {
      // JSON.stringify returns undefined for functions, symbols, etc.
      return strdup_alloc("null");
    }

    return strdup_alloc(jsonStr.getString(runtime).utf8(runtime));
  } catch (const jsi::JSError& e) {
    if (error_out) {
      *error_out = strdup_alloc(e.what());
    }
    return nullptr;
  } catch (const std::exception& e) {
    if (error_out) {
      *error_out = strdup_alloc(e.what());
    }
    return nullptr;
  }
}

void hermes_free_string(char* s) {
  free(s);
}

void hermes_destroy_runtime(HermesRuntime* rt) {
  delete rt;
}

} // extern "C"
