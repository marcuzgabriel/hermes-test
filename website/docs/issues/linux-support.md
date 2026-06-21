---
title: Linux support
---

# Linux support

## Status

Linux is supported for hermes-test, including CI usage.

## What to know

- Linux is a supported runtime target
- Intl behavior on Linux may differ from macOS due to Hermes backend differences
- hermes-test includes targeted fallbacks to stabilize known Linux-specific Intl gaps

macOS remains the closest reference environment for iOS parity, but Linux runs are fully valid for automation and team workflows.

## Practical guidance

1. Run tests on Linux CI as normal.
2. If locale-specific assertions are brittle, verify with the Intl notes page.
3. Prefer deterministic assertions around formatted values when possible.

## References

- [README platform section](https://github.com/marcuzgabriel/hermes-test#platform-and-intl-support)
- [Intl observations](./intl-observations)
