use std::env;
use std::path::PathBuf;

fn main() {
    // V8 engine: locate and embed ICU data file for Intl support
    if env::var("CARGO_FEATURE_V8_ENGINE").is_ok() {
        // Find the v8 crate's ICU data file
        let out_dir = env::var("OUT_DIR").unwrap();
        // The v8 crate stores its source in the cargo registry
        // Walk up from OUT_DIR to find the registry, then locate the ICU data
        let cargo_home = env::var("CARGO_HOME")
            .unwrap_or_else(|_| format!("{}/.cargo", env::var("HOME").unwrap()));
        let registry = PathBuf::from(&cargo_home).join("registry/src");

        // Find the v8 crate directory
        let icu_data_path = find_icu_data(&registry)
            .unwrap_or_else(|| {
                eprintln!("WARNING: Could not find V8 ICU data file (icudtl.dat)");
                eprintln!("  Intl functions (toLocaleDateString, etc.) will not work.");
                // Create an empty file as fallback
                let fallback = PathBuf::from(&out_dir).join("empty_icu.dat");
                std::fs::write(&fallback, &[]).unwrap();
                fallback.to_string_lossy().to_string()
            });

        println!("cargo:rustc-env=V8_ICU_DATA_PATH={icu_data_path}");
    }

    // Hermes C++ bridge is only needed when the hermes-engine feature is enabled.
    if env::var("CARGO_FEATURE_HERMES_ENGINE").is_ok() {
        build_hermes_bridge();
    }
}

fn find_icu_data(registry: &PathBuf) -> Option<String> {
    // Search for v8-*/third_party/icu/flutter_desktop/icudtl.dat
    if let Ok(index_dirs) = std::fs::read_dir(registry) {
        for index_dir in index_dirs.flatten() {
            let path = index_dir.path();
            if let Ok(crate_dirs) = std::fs::read_dir(&path) {
                for crate_dir in crate_dirs.flatten() {
                    let name = crate_dir.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with("v8-") {
                        let icu_path = crate_dir.path()
                            .join("third_party/icu/flutter_desktop/icudtl.dat");
                        if icu_path.exists() {
                            return Some(icu_path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

fn build_hermes_bridge() {
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

    // Compile hermes_compile.cpp — in-process bytecode compilation
    let mut compile_build = cc::Build::new();
    compile_build.cpp(true).std("c++17").file(bridge_dir.join("src/hermes_compile.cpp"));
    for inc in &common_includes { compile_build.include(inc); }
    compile_build.include(hermes_dir.join("lib"));
    compile_build
        .flag("-Wno-non-virtual-dtor")
        .flag("-fexceptions")
        .flag("-frtti")
        .compile("hermes_compile");

    // Link Hermes static libraries
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
        hermes_build.join("lib/Platform/Intl"),
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

    let libs = [
        "hermesvm",
        "hermesVMRuntime",
        "hermesFrontend",
        "hermesCompilerDriver",
        "hermesHBCBackend",
        "hermesOptimizer",
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
        "hermesPlatformIntl",
        "hermesBCP47Parser",
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
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreServices");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=icuuc");
        println!("cargo:rustc-link-lib=icui18n");
        println!("cargo:rustc-link-lib=icudata");
    }

    println!("cargo:rerun-if-changed={}", bridge_dir.join("src/bridge.cpp").display());
    println!("cargo:rerun-if-changed={}", bridge_dir.join("src/vm_eval.cpp").display());
    println!("cargo:rerun-if-changed={}", bridge_dir.join("include/bridge.h").display());
}
