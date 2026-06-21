---
title: API overview
---

# API overview

This section is based on the runtime harness and published type surface.

## Modules

- [test](./test)
- [expect](./expect)
- [spy](./spy)
- [mock](./mock)
- [hooks](./hooks)
- [render](./render)
- [store](./store)
- [timers](./timers)

## Global helper

`ht` is available globally for module mocking directives:

- `ht.mock(path, factory)`
- `ht.mock.fetch(...)`
- `ht.unmock(path)`
