#include "bridge.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>

namespace jsi = facebook::jsi;

struct HermesRuntime {
  std::unique_ptr<facebook::hermes::HermesRuntime> rt;
};

// Format a jsi::Value to a string for console output
static std::string valueToString(jsi::Runtime& runtime, const jsi::Value& val) {
  if (val.isUndefined()) return "undefined";
  if (val.isNull()) return "null";
  if (val.isBool()) return val.getBool() ? "true" : "false";
  if (val.isNumber()) {
    double d = val.getNumber();
    if (d == static_cast<int64_t>(d))
      return std::to_string(static_cast<int64_t>(d));
    return std::to_string(d);
  }
  if (val.isString()) return val.getString(runtime).utf8(runtime);
  if (val.isObject()) {
    // Use JSON.stringify for objects/arrays
    try {
      auto json = runtime.global().getPropertyAsObject(runtime, "JSON");
      auto stringify = json.getPropertyAsFunction(runtime, "stringify");
      auto result = stringify.call(runtime, val, jsi::Value::undefined(), jsi::Value(2));
      if (!result.isUndefined())
        return result.getString(runtime).utf8(runtime);
    } catch (...) {}
    return "[Object]";
  }
  return "[unknown]";
}

// Install console.log/warn/error on the global object
static void installConsole(jsi::Runtime& runtime) {
  auto console = jsi::Object(runtime);

  auto makeLogger = [](FILE* stream) {
    return [stream](jsi::Runtime& rt, const jsi::Value&,
                     const jsi::Value* args, size_t count) -> jsi::Value {
      for (size_t i = 0; i < count; i++) {
        if (i > 0) fprintf(stream, " ");
        fprintf(stream, "%s", valueToString(rt, args[i]).c_str());
      }
      fprintf(stream, "\n");
      fflush(stream);
      return jsi::Value::undefined();
    };
  };

  console.setProperty(runtime, "log",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "log"), 0, makeLogger(stderr)));
  console.setProperty(runtime, "warn",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "warn"), 0, makeLogger(stderr)));
  console.setProperty(runtime, "error",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "error"), 0, makeLogger(stderr)));
  console.setProperty(runtime, "info",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "info"), 0, makeLogger(stderr)));

  runtime.global().setProperty(runtime, "console", console);
}

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
    installConsole(*wrapper->rt);
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
