#include "bridge.h"

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <sys/stat.h>

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
  // Rust reads globalThis.__HT_logs after eval and prints them cleanly.
  auto logs = jsi::Array(runtime, 0);
  runtime.global().setProperty(runtime, "__HT_logs", std::move(logs));

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
        auto logsVal = rt.global().getProperty(rt, "__HT_logs");
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

  // Install __HT_print — writes directly to stderr during eval.
  // Used by the harness to stream test results in real-time.
  runtime.global().setProperty(runtime, "__HT_print",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "__HT_print"), 1,
      [](jsi::Runtime& rt, const jsi::Value&,
         const jsi::Value* args, size_t count) -> jsi::Value {
        if (count > 0 && args[0].isString()) {
          auto msg = args[0].getString(rt).utf8(rt);
          fprintf(stderr, "%s", msg.c_str());
          fflush(stderr);
        }
        return jsi::Value::undefined();
      }));

  // Install __HT_readFile — read file contents (for snapshot testing)
  runtime.global().setProperty(runtime, "__HT_readFile",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "__HT_readFile"), 1,
      [](jsi::Runtime& rt, const jsi::Value&,
         const jsi::Value* args, size_t count) -> jsi::Value {
        if (count == 0 || !args[0].isString())
          return jsi::Value::null();
        auto path = args[0].getString(rt).utf8(rt);
        FILE* f = fopen(path.c_str(), "r");
        if (!f) return jsi::Value::null();
        fseek(f, 0, SEEK_END);
        long size = ftell(f);
        fseek(f, 0, SEEK_SET);
        std::string content(size, '\0');
        fread(&content[0], 1, size, f);
        fclose(f);
        return jsi::String::createFromUtf8(rt, content);
      }));

  // Install __HT_writeFile — write file contents (for snapshot testing)
  runtime.global().setProperty(runtime, "__HT_writeFile",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "__HT_writeFile"), 2,
      [](jsi::Runtime& rt, const jsi::Value&,
         const jsi::Value* args, size_t count) -> jsi::Value {
        if (count < 2 || !args[0].isString() || !args[1].isString())
          return jsi::Value(false);
        auto path = args[0].getString(rt).utf8(rt);
        auto content = args[1].getString(rt).utf8(rt);
        // Create parent directories if needed
        auto lastSlash = path.rfind('/');
        if (lastSlash != std::string::npos) {
          auto dir = path.substr(0, lastSlash);
          // Simple recursive mkdir (POSIX)
          std::string built;
          for (auto& ch : dir) {
            built += ch;
            if (ch == '/') mkdir(built.c_str(), 0755);
          }
          mkdir(dir.c_str(), 0755);
        }
        FILE* f = fopen(path.c_str(), "w");
        if (!f) return jsi::Value(false);
        fwrite(content.c_str(), 1, content.size(), f);
        fclose(f);
        return jsi::Value(true);
      }));

  // Install __HT_drain — calls Hermes's real microtask queue drain.
  // This replaces the hacky polyfill loop with a single native call.
  runtime.global().setProperty(runtime, "__HT_drain",
    jsi::Function::createFromHostFunction(runtime,
      jsi::PropNameID::forAscii(runtime, "__HT_drain"), 0,
      [](jsi::Runtime& rt, const jsi::Value&,
         const jsi::Value*, size_t) -> jsi::Value {
        rt.drainMicrotasks();
        return jsi::Value::undefined();
      }));
}

extern "C" {

HermesRuntime* hermes_create_runtime(void) {
  try {
    auto wrapper = new HermesRuntime();
    auto gcConfig = ::hermes::vm::GCConfig::Builder()
        .withMaxHeapSize(512 * 1024 * 1024)  // 512MB
        .withInitHeapSize(32 * 1024 * 1024)   // 32MB initial
        .build();
    auto config = ::hermes::vm::RuntimeConfig::Builder()
        .withES6Class(true)
        .withEnableBlockScoping(true)
        .withMicrotaskQueue(true)
        .withMaxNumRegisters(1024 * 1024)
        .withGCConfig(gcConfig)
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

    // All evals use JSI. For class bundles, Rust precompiles via hermesc.
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
