// Direct VM evaluation — bypasses JSI evaluateJavaScript bug where ES6
// class transformation fails for bundles >64KB. Uses Runtime::run() which
// works correctly (same path as the hermes CLI).

#include <hermes/VM/Runtime.h>
// CompileFlags is defined in BytecodeProviderFromSrc.h but that header
// transitively includes LLVH IR headers with generated .inc files we don't
// have. Since Runtime.h only forward-declares it, we replicate the struct
// here with just the fields we need. The memory layout must match exactly.
#include <hermes/Utils/Options.h>  // for OutputFormatKind
#include "llvh/ADT/Optional.h"     // for llvh::Optional

namespace hermes { namespace hbc {
struct CompileFlags {
  bool debug{false};
  bool lazy{false};
  bool enableBlockScoping{false};
  unsigned preemptiveFileCompilationThreshold{1 << 16};
  unsigned preemptiveFunctionCompilationThreshold{160};
  bool strict{false};
  llvh::Optional<bool> staticBuiltins;
  bool verifyIR{false};
  bool emitAsyncBreakCheck{false};
  bool includeLibHermes{true};
  bool instrumentIR{false};
  bool enableGenerator{true};
  bool enableES6Classes{false};
  OutputFormatKind format{Execute};
};
}} // namespace hermes::hbc

#include <cstdlib>
#include <cstring>
#include <string>

static char* vm_strdup(const std::string& s) {
  char* out = static_cast<char*>(malloc(s.size() + 1));
  if (out) {
    memcpy(out, s.data(), s.size());
    out[s.size()] = '\0';
  }
  return out;
}

extern "C" {

/// Run JS source via VM Runtime::run() — correctly handles ES6 classes.
/// vm_runtime_ptr comes from HermesRuntime::getVMRuntimeUnsafe().
/// Returns 0 on success, 1 on error (error message in error_out).
int hermes_vm_run(
    void* vm_runtime_ptr,
    const char* source,
    size_t source_len,
    const char* source_url,
    char** error_out) {
  if (error_out) *error_out = nullptr;

  if (!vm_runtime_ptr) {
    if (error_out) *error_out = vm_strdup("vm_runtime_ptr is null");
    return 1;
  }
  if (!source || source_len == 0) {
    if (error_out) *error_out = vm_strdup("empty source");
    return 1;
  }

  auto* runtime = static_cast<::hermes::vm::Runtime*>(vm_runtime_ptr);

  ::hermes::hbc::CompileFlags flags;
  flags.enableES6Classes = true;
  flags.enableBlockScoping = true;
  flags.enableGenerator = true;
  flags.includeLibHermes = false;

  // Force Hermes to copy the source buffer (prevents SIGBUS from non-owning ref)
  flags.lazy = true;
  flags.preemptiveFileCompilationThreshold = 0;
  flags.preemptiveFunctionCompilationThreshold = 0;

  try {
    auto result = runtime->run(
        llvh::StringRef(source, source_len),
        source_url ? source_url : "eval",
        flags);

    if (result.getStatus() == ::hermes::vm::ExecutionStatus::EXCEPTION) {
      std::string msg = "JS exception";
      runtime->clearThrownValue();
      if (error_out) *error_out = vm_strdup(msg);
      return 1;
    }
  } catch (const std::exception &e) {
    if (error_out) *error_out = vm_strdup(std::string("C++ exception: ") + e.what());
    return 1;
  } catch (...) {
    if (error_out) *error_out = vm_strdup("Unknown C++ exception in vm_run");
    return 1;
  }

  return 0;
}

} // extern "C"
