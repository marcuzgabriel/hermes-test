use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir.parent().unwrap().parent().unwrap();
    let hermes_dir = repo_root.join("vendor/hermes");
    let hermes_build = hermes_dir.join("build");
    let bridge_dir = repo_root.join("crates/hermes-bridge");

    // Compile the bridge C++ file and jsi.cpp (statically, to avoid dylib dep)
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file(bridge_dir.join("src/bridge.cpp"))
        .file(hermes_dir.join("API/jsi/jsi/jsi.cpp"))
        .include(bridge_dir.join("include"))
        .include(hermes_dir.join("API"))
        .include(hermes_dir.join("API/hermes"))
        .include(hermes_dir.join("API/jsi"))
        .include(hermes_dir.join("public"))
        .include(hermes_dir.join("include"))
        .include(hermes_dir.join("external/llvh/include"))
        .include(hermes_build.join("external/llvh/include"))
        .include(hermes_build.join("include"))
        .flag("-Wno-non-virtual-dtor")
        .flag("-fexceptions")
        .flag("-frtti")
        .compile("hermes_bridge");

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
    // libhermes contains hermes.cpp (makeHermesRuntime) and links jsi
    let libs = [
        "hermes",
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
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=framework=Foundation");

    println!("cargo:rerun-if-changed={}", bridge_dir.join("src/bridge.cpp").display());
    println!("cargo:rerun-if-changed={}", bridge_dir.join("include/bridge.h").display());
}
