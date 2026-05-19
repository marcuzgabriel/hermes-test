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

static char* strdup_alloc(const std::string& s) {
  char* out = static_cast<char*>(malloc(s.size() + 1));
  if (out) {
    memcpy(out, s.data(), s.size());
    out[s.size()] = '\0';
  }
  return out;
}

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

static void installConsole(jsi::Runtime& runtime) {
  // Store logs in a JS array — avoids mixing with Hermes's internal stderr output.
  // Rust reads globalThis.__consoleLogs after eval and prints them cleanly.
  auto logs = jsi::Array(runtime, 0);
  runtime.global().setProperty(runtime, "__consoleLogs", std::move(logs));

  auto console = jsi::Object(runtime);
  auto makeLogger = [](const char* level) {
    return [level](jsi::Runtime& rt, const jsi::Value&,
                     const jsi::Value* args, size_t count) -> jsi::Value {
      try {
        std::string msg;
        for (size_t i = 0; i < count; i++) {
          if (i > 0) msg += " ";
          msg += valueToString(rt, args[i]);
        }
        auto logsVal = rt.global().getProperty(rt, "__consoleLogs");
        if (logsVal.isObject()) {
          auto logsArr = logsVal.getObject(rt).getArray(rt);
          auto len = logsArr.size(rt);
          auto entry = jsi::Object(rt);
          entry.setProperty(rt, "level", jsi::String::createFromUtf8(rt, level));
          entry.setProperty(rt, "message", jsi::String::createFromUtf8(rt, msg));
          logsArr.setValueAtIndex(rt, len, std::move(entry));
        }
      } catch (...) {
        // Silently drop logs that fail to serialize (e.g. circular refs)
      }
      return jsi::Value::undefined();
    };
  };
  console.setProperty(runtime, "log",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "log"), 0, makeLogger("log")));
  console.setProperty(runtime, "warn",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "warn"), 0, makeLogger("warn")));
  console.setProperty(runtime, "error",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "error"), 0, makeLogger("error")));
  console.setProperty(runtime, "info",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "info"), 0, makeLogger("info")));
  runtime.global().setProperty(runtime, "console", console);

  // Install __drainMicrotasks — calls Hermes's real microtask queue drain.
  // This replaces the hacky polyfill loop with a single native call.
  runtime.global().setProperty(runtime, "__drainMicrotasks",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "__drainMicrotasks"), 0,
      [](jsi::Runtime& rt, const jsi::Value&,
         const jsi::Value*, size_t) -> jsi::Value {
        rt.drainMicrotasks();
        return jsi::Value::undefined();
      }));
}

// Defined in vm_eval.cpp — direct VM execution for large bundles
extern "C" int hermes_vm_run(
    void* vm_runtime_ptr,
    const char* source, size_t source_len,
    const char* source_url, char** error_out);

extern "C" {

HermesRuntime* hermes_create_runtime(void) {
  try {
    auto wrapper = new HermesRuntime();
    auto config = ::hermes::vm::RuntimeConfig::Builder()
        .withES6Class(true)
        .withEnableBlockScoping(true)
        .withMaxNumRegisters(1024 * 1024)
        .build();
    wrapper->rt = facebook::hermes::makeHermesRuntime(config);
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

    // Large bundles (>60KB): use VM Runtime::run() to avoid JSI class bug
    if (source_len > 60000) {
      char* vm_error = nullptr;
      int rc = hermes_vm_run(
          runtime.getVMRuntimeUnsafe(),
          source, source_len,
          source_url ? source_url : "eval",
          &vm_error);
      if (rc != 0) {
        if (error_out && vm_error) *error_out = vm_error;
        else free(vm_error);
        return nullptr;
      }
      return strdup_alloc("null");
    }

    // Small scripts: normal JSI path
    auto result = runtime.evaluateJavaScript(
        std::make_shared<jsi::StringBuffer>(
            std::string(source, source_len)),
        source_url ? source_url : "eval");

    auto json = runtime.global().getPropertyAsObject(runtime, "JSON");
    auto stringify = json.getPropertyAsFunction(runtime, "stringify");
    auto jsonStr = stringify.call(runtime, result);

    if (jsonStr.isUndefined()) return strdup_alloc("null");
    return strdup_alloc(jsonStr.getString(runtime).utf8(runtime));
  } catch (const jsi::JSError& e) {
    if (error_out) *error_out = strdup_alloc(e.what());
    return nullptr;
  } catch (const std::exception& e) {
    if (error_out) *error_out = strdup_alloc(e.what());
    return nullptr;
  }
}

void hermes_free_string(char* s) { free(s); }
void hermes_destroy_runtime(HermesRuntime* rt) { delete rt; }

} // extern "C"
