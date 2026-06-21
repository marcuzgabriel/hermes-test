---
title: Coverage model
---

# Coverage model

## How coverage works

Use:

```bash
hermes-test --coverage
```

The runner instruments bundle output with Istanbul-style counters, then executes through the Hermes bytecode path.

## Why bytecode path matters

Coverage stability depends on running instrumented output through the same bytecode-oriented execution model used by normal test runs.

That keeps behavior consistent between:

- normal test execution
- coverage execution

## Configurable threshold

Set coverage gate in `hermes-test.config.json`:

```json
{
  "coverageThreshold": 65
}
```

If total coverage is below threshold, the run fails.

## More detail

- [Coverage approaches reference](../references/coverage-approaches)
- [Benchmark coverage results](../benchmarks/overview)
