# Week 1 — Paste this into Claude Code

Copy the prompt below into your first Claude Code session in this repo. It tells Claude Code exactly what to build, what to measure, and what counts as "done."

**Realistic time estimate**: 4-6 days of focused work, roughly 2 weekends at nights-and-weekends pace. Most of the time is in two places: getting Hermes to build cleanly (0.5-1 day) and getting the `cxx` bridge working (1-2 days). Everything else is mechanical.

**If you've never used Rust + C++ interop before**: that's expected. Claude Code can write most of the bridge code from a spec; your job is direction and debugging, not memorizing Rust syntax. Lean on Claude Code aggressively for the boilerplate.

---

## Prompt for Claude Code

> Read `.claude/skills/metro-test/SKILL.md` and all files in `.claude/references/`. Then read `.claude/references/roadmap.md` carefully — we are working on Week 1.
>
> Your task this session is the Week 1 deliverable: a Rust CLI that embeds Hermes via a C++ shim, evaluates a hardcoded JS string, and prints a JSON result. The gate is a sub-1-second cold start on a trivial test.
>
> Build steps, in order:
>
> 1. Set up the workspace:
>    - `crates/hermes-bridge/` — C++ shim with `extern "C"` interface
>    - `crates/metro-test-cli/` — Rust binary
>    - `bench/fixtures/trivial.test.js` — placeholder, just a string for now
>    - Top-level `Cargo.toml` workspace
>
> 2. Vendor or fetch Hermes:
>    - Use `facebook/hermes` source, build with `cmake -DCMAKE_BUILD_TYPE=Release`
>    - Document the build steps in `crates/hermes-bridge/README.md`
>    - If vendoring is too heavy for v0, document the system-install path instead and add a `build.rs` that finds the local Hermes install
>
> 3. Write the C++ shim (`crates/hermes-bridge/include/bridge.h` + `src/bridge.cpp`):
>    - `hermes_create_runtime() -> Runtime*`
>    - `hermes_eval(Runtime*, const char* source, const char* url) -> const char*` (returns JSON-stringified result or error object)
>    - `hermes_destroy_runtime(Runtime*)`
>    - Use JSI (`facebook::jsi`) and `facebook::hermes::makeHermesRuntime()`
>    - Wrap evaluations in try/catch, return `{error: "..."}` JSON on `jsi::JSError`
>    - Use `JSON.stringify` via JSI to serialize the result value
>
> 4. Write the Rust CLI (`crates/metro-test-cli/`):
>    - Use `cxx` for the C++ bridge
>    - `clap` for argument parsing — for week 1, just take a file path
>    - `serde_json` for parsing the result
>    - `main.rs` should: create runtime → read file → eval → parse JSON → print pass/fail
>    - Hardcode a JS test harness inline: a function `runTest(name, fn)` that wraps in try/catch and returns `{name, status, error?}`, plus a simple `expect(actual).toBe(expected)`
>
> 5. Write `bench/fixtures/trivial.test.js`:
>    - One call to `runTest('adds two numbers', () => expect(1 + 1).toBe(2))`
>    - End with the array of results so `JSON.stringify` of the last expression captures them
>
> 6. Verify it runs end-to-end:
>    - `cargo run -p metro-test-cli -- bench/fixtures/trivial.test.js`
>    - Should print something like `✓ adds two numbers`
>
> 7. Benchmark (the gate):
>    - Install `hyperfine` if not present
>    - Run: `hyperfine --warmup 1 --runs 10 'cargo run --release -p metro-test-cli -- bench/fixtures/trivial.test.js'`
>    - Also run: `hyperfine --warmup 1 --runs 10 'npx jest bench/fixtures-jest/trivial.test.js'` (create the Jest equivalent first)
>    - Record both in `BENCHMARKS.md` using the format from `.claude/references/benchmarks.md`
>
> Gate criteria:
> - Cold start mean under 1000ms — pass
> - Cold start mean between 1000-2000ms — surface to me, we discuss
> - Cold start mean over 2000ms — stop, the architecture has a problem we need to debug before continuing
>
> What I don't want you to do this week:
> - Don't pull in Metro yet (week 2)
> - Don't implement `useMock` or `renderHook` yet (week 3)
> - Don't write a pretty reporter yet (week 4)
> - Don't add a watch mode yet (week 4)
> - Don't expand the matcher set beyond `toBe` and maybe `toEqual` (week 2)
>
> If you hit something the references don't cover, ask before deciding. The references are settled; everything else is open.
>
> When you're done, give me:
> 1. A summary of what was built
> 2. The benchmark numbers in BENCHMARKS.md
> 3. Whether the gate passed
> 4. Any decisions you made that should be added to `decisions.md`

---

## After Week 1

If the gate passes, paste the Week 2 prompt (to be written based on what we learned). If it fails by less than 2x, we discuss the bottleneck. If it fails by 2x+, we revisit the architecture before continuing.
