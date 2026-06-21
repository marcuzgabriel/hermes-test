---
title: timers
---

# timers API

```ts
import {
  useFakeTimers,
  useRealTimers,
  advanceTimersByTime,
  runAllTimers,
  getTimerCount,
  advanceTimersToNextTimer,
} from 'hermes-test';
```

## Usage

```ts
useFakeTimers();
advanceTimersByTime(1000);
advanceTimersToNextTimer();
runAllTimers();
expect(getTimerCount()).toBe(0);
useRealTimers();
```
