---
title: Intl observations
---

# Intl observations

## Current observation

Hermes Intl behavior differs by platform backend:

- Apple platforms: strong native Intl behavior
- Android: strong native Intl behavior
- Linux desktop/CI: some Intl paths are incomplete in Hermes upstream

## Current handling in hermes-test

For Linux-specific gaps, hermes-test applies targeted runtime fallbacks where behavior is clearly broken, including:

- `Intl.NumberFormat` / `toLocaleString` numeric formatting paths
- locale casing safeguards (`toLocaleLowerCase` / `toLocaleUpperCase`)

These fallbacks are intentionally narrow and deterministic.

## Scope

- Not a full locale engine replacement
- Designed to keep CI/test behavior stable
- Native behavior is preserved where it already works

## References

- [README platform section](https://github.com/marcuzgabriel/hermes-test#platform-and-intl-support)
- [Challenges reference](../references/challenges)
