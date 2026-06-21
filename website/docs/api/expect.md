---
title: expect
---

# expect API

## Core matchers

```ts
expect(value).toBe(expected);
expect(value).toEqual(expected);
expect(value).toBeTruthy();
expect(value).toBeFalsy();
expect(value).toBeDefined();
expect(value).toBeUndefined();
expect(value).toBeNull();
expect(value).toContain(item);
expect(value).toContainEqual(item);
expect(value).toMatch(/regex/);
expect(value).toBeCloseTo(number, precision);
expect(value).toMatchObject(partial);
expect(value).toMatchSnapshot();
expect(fn).toThrow('message');
expect(value).not.toBe(other);
```

## Spy matchers

```ts
expect(spyFn).wasCalled();
expect(spyFn).wasCalledOnce();
expect(spyFn).wasCalledTimes(2);
expect(spyFn).wasCalledWith('a', 1);
expect(spyFn).wasLastCalledWith('b');
expect(spyFn).wasNeverCalled();

// Jest aliases
expect(spyFn).toHaveBeenCalled();
expect(spyFn).toHaveBeenCalledTimes(2);
expect(spyFn).toHaveBeenCalledWith('a', 1);
expect(spyFn).toHaveBeenLastCalledWith('b');
```

## Element matchers

```ts
expect(node).toBeRendered();
expect(node).toHaveTextContent('Hello');
expect(node).toContainElement(child);
expect(node).toBeEmpty();
expect(node).toHaveDisplayValue('abc');
expect(node).toHaveProp('disabled', true);
expect(node).toHaveStyle({opacity: 0.5});
expect(node).toBeEnabled();
expect(node).toBeDisabled();
expect(node).toBeVisible();
```

## Async matchers

```ts
await expect(promise).resolves.toBe(value);
await expect(promise).rejects.toThrow('error');
```

## Asymmetric helpers

```ts
expect.anything();
expect.any(String);
expect.objectContaining({id: 1});
expect.arrayContaining([1, 2]);
expect.stringContaining('sub');
expect.stringMatching(/abc/);
```
