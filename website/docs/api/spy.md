---
title: spy
---

# spy API

## Create spies

```ts
import {spy, spyOn, clearAllMocks} from 'hermes-test';

const fn = spy();
const sum = spy((a: number, b: number) => a + b);
const methodSpy = spyOn(obj, 'method');
```

## Spy controls

```ts
fn.mockImplementation(() => 1);
fn.mockImplementationOnce(() => 2);
fn.mockReturnValue(3);
fn.mockReturnValueOnce(4);
fn.mockResolvedValue({ok: true});
fn.mockResolvedValueOnce({ok: false});
fn.mockRejectedValue(new Error('fail'));
fn.mockRejectedValueOnce(new Error('fail once'));
fn.mockClear();
fn.mockReset();
fn.mockRestore();
clearAllMocks();
```

## Spy state

```ts
fn.calls;        // call arguments by invocation
fn.callCount;    // number of calls
fn.returnValues; // return values
```
