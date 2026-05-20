// In-process Hermes bytecode compilation — replaces hermesc subprocess.
// Compiles JS source → Hermes bytecode with full ES6 class support.
// Zero subprocess overhead, zero temp file I/O.

#include <hermes/BCGen/HBC/BytecodeProviderFromSrc.h>
#include <hermes/BCGen/HBC/BytecodeStream.h>
#include <hermes/Support/MemoryBuffer.h>
#include "llvh/Support/SHA1.h"

#include <cstdlib>
#include <cstring>
#include <string>

static char* compile_strdup(const std::string& s) {
  char* out = static_cast<char*>(malloc(s.size() + 1));
  if (out) {
    memcpy(out, s.data(), s.size());
    out[s.size()] = '\0';
  }
  return out;
}

extern "C" {

/// Compile JS source to Hermes bytecode in-process.
/// Returns 0 on success, 1 on error.
int hermes_compile(
    const char* source,
    size_t source_len,
    const char* source_url,
    unsigned char** out_bytecode,
    size_t* out_size,
    char** error_out) {

  if (error_out) *error_out = nullptr;
  if (out_bytecode) *out_bytecode = nullptr;
  if (out_size) *out_size = 0;

  auto buf = llvh::MemoryBuffer::getMemBufferCopy(
      llvh::StringRef(source, source_len),
      source_url ? source_url : "input.js");

  hermes::hbc::CompileFlags flags;
  flags.enableES6Classes = true;
  flags.enableBlockScoping = true;
  flags.enableGenerator = true;
  flags.includeLibHermes = false;

  auto result = hermes::hbc::BCProviderFromSrc::createBCProviderFromSrc(
      std::make_unique<hermes::OwnedMemoryBuffer>(std::move(buf)),
      source_url ? source_url : "input.js",
      flags);

  if (!result.first) {
    if (error_out) *error_out = compile_strdup(result.second);
    return 1;
  }

  // Serialize to bytecode
  std::string bytecodeStr;
  llvh::raw_string_ostream OS(bytecodeStr);

  // Zero hash — not needed for test execution
  hermes::SHA1 hash = {};

  hermes::hbc::BytecodeSerializer serializer(OS);
  serializer.serialize(*result.first->getBytecodeModule(), hash);
  OS.flush();

  *out_size = bytecodeStr.size();
  *out_bytecode = static_cast<unsigned char*>(malloc(bytecodeStr.size()));
  if (*out_bytecode) {
    memcpy(*out_bytecode, bytecodeStr.data(), bytecodeStr.size());
  }

  return 0;
}

} // extern "C"
