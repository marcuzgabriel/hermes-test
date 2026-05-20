use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.parent().unwrap().parent().unwrap();
    let hermes_dir = repo_root.join("vendor/hermes");
    let hermes_build = hermes_dir.join("build");
    let bridge_dir = repo_root.join("crates/hermes-bridge");

    let common_includes: Vec<std::path::PathBuf> = vec![
        bridge_dir.join("include"),
        hermes_dir.join("API"),
        hermes_dir.join("API/hermes"),
        hermes_dir.join("API/jsi"),
        hermes_dir.join("public"),
        hermes_dir.join("include"),
        hermes_dir.join("lib"),
        hermes_dir.join("external/llvh/include"),
        hermes_build.join("external/llvh/include"),
        hermes_build.join("include"),
    ];

    // Compile the bridge C++ file and jsi.cpp (statically, to avoid dylib dep)
    let mut bridge_build = cc::Build::new();
    bridge_build
        .cpp(true)
        .std("c++17")
        .file(bridge_dir.join("src/bridge.cpp"))
        .file(hermes_dir.join("API/jsi/jsi/jsi.cpp"));
    for inc in &common_includes {
        bridge_build.include(inc);
    }
    bridge_build
        .flag("-Wno-non-virtual-dtor")
        .flag("-fexceptions")
        .flag("-frtti")
        .compile("hermes_bridge");

    // Compile vm_eval.cpp — uses VM Runtime::run() for large bundles
    let mut vm_build = cc::Build::new();
    vm_build.cpp(true).std("c++17").file(bridge_dir.join("src/vm_eval.cpp"));
    for inc in &common_includes { vm_build.include(inc); }
    vm_build
        .define("HERMESVM_ALLOW_COMPRESSED_POINTERS", None)
        .define("HERMESVM_ALLOW_CONCURRENT_GC", None)
        .define("HERMESVM_ALLOW_INLINE_ASM", None)
        .define("HERMESVM_GC_HADES", None)
        .define("HERMESVM_HEAP_SEGMENT_SIZE_KB", "4096")
        .define("HERMESVM_INDIRECT_THREADING", None)
        .flag("-Wno-non-virtual-dtor")
        .flag("-fexceptions")
        .flag("-frtti")
        .compile("hermes_vm_eval");

    // Link Hermes static libraries (order matters for static linking)
    let lib_dirs = [
        hermes_build.join("API/hermes"),
        hermes_build.join("lib/VM"),
        hermes_build.join("lib"),
        hermes_build.join("lib/BCGen/HBC"),
        hermes_build.join("lib/BCGen"),
        hermes_build.join("lib/CompilerDriver"),
        hermes_build.join("lib/Inst"),
        hermes_build.join("lib/FrontEndDefs"),
        hermes_build.join("lib/Parser"),
        hermes_build.join("lib/AST"),
        hermes_build.join("lib/SourceMap"),
        hermes_build.join("lib/FlowParser"),
        hermes_build.join("lib/AST2JS"),
        hermes_build.join("lib/Support"),
        hermes_build.join("lib/ADT"),
        hermes_build.join("lib/Platform"),
        hermes_build.join("lib/Platform/Unicode"),
        hermes_build.join("lib/Regex"),
        hermes_build.join("lib/InternalBytecode"),
        hermes_build.join("public/hermes/Public"),
        hermes_build.join("external/dtoa"),
        hermes_build.join("external/llvh/lib/Support"),
        hermes_build.join("external/llvh/lib/Demangle"),
    ];

    for dir in &lib_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    // Link order matters: dependents before dependencies
    // Hermes V1 (rn/0.84): libhermesvm replaces libhermes
    let libs = [
        "hermesvm",
        "hermesVMRuntime",
        "hermesFrontend",
        "hermesOptimizer",
        "hermesCompilerDriver",
        "hermesHBCBackend",
        "hermesBackend",
        "hermesInst",
        "hermesFrontEndDefs",
        "hermesParser",
        "hermesAST",
        "hermesFlowParser",
        "hermesAST2JS",
        "hermesSourceMap",
        "hermesInternalBytecode",
        "hermesRegex",
        "hermesPlatformUnicode",
        "hermesPlatform",
        "hermesSupport",
        "hermesADT",
        "hermesPublic",
        "dtoa",
        "LLVHSupport",
        "LLVHDemangle",
    ];

    for lib in &libs {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    // System libraries
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=Foundation");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
        // Hermes uses ICU for Unicode on Linux (macOS uses Foundation instead)
        println!("cargo:rustc-link-lib=icuuc");
        println!("cargo:rustc-link-lib=icui18n");
        println!("cargo:rustc-link-lib=icudata");
    }

    println!("cargo:rerun-if-changed={}", bridge_dir.join("src/bridge.cpp").display());
    println!("cargo:rerun-if-changed={}", bridge_dir.join("src/vm_eval.cpp").display());
    println!("cargo:rerun-if-changed={}", bridge_dir.join("include/bridge.h").display());
}
