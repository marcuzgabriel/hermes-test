---
title: Tooling rationale
---

# Tooling rationale

## Why React reconciler

hermes-test uses a reconciler-based render layer so tests can run component logic without a DOM and still support:

- render trees
- query APIs (`getByText`, `getByTestId`, `getByType`, etc.)
- event simulation (`fireEvent.press`, `changeText`, `scroll`)
- snapshots (`toMatchSnapshot`)

This gives React Native-focused component testing behavior in Hermes runtime.

## Why proxy-heavy mocking

The mock system relies on proxy/wrapper patterns because ESM imports are statically resolved at bundle time.

Proxy wrappers allow runtime interception for:

- per-file mock scoping
- late mock registration (`ht.mock(...)`)
- fallback to real modules when no override exists

This is the basis for shadow wrappers and package shims.

## Why this combo

React reconciler + proxy isolation gives:

- realistic RN test behavior
- deterministic single-runtime execution
- practical mocking across large codebases without Jest worker isolation
