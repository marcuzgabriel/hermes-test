// Thin re-export from the hermes-test harness runtime.
// The harness is eval'd before the user bundle, so globalThis.__HT is always available.
// Users import from 'hermes-test' instead of accessing globalThis directly.

const ht = (globalThis as any).__HT;

export const test = ht.test;
export const group = ht.group;
export const expect = ht.expect;
export const spy = ht.spy;
export const spyOn = ht.spyOn;
export const beforeEach = ht.beforeEach;
export const afterEach = ht.afterEach;
export const beforeAll = ht.beforeAll;
export const afterAll = ht.afterAll;
export const renderHook = ht.renderHook;
export const act = ht.act;
export const waitFor = ht.waitFor;
export const mockModule = ht.mockModule;
export const useMock = ht.useMock;
export const mockFetch = ht.mockFetch;
export const mockFetchUse = ht.mockFetchUse;
export const mockFetchReset = ht.mockFetchReset;
export const mockFetchClear = ht.mockFetchClear;
export const http = ht.http;
export const HttpResponse = ht.HttpResponse;
export const flushAsync = ht.flushAsync;
export const useFakeTimers = ht.useFakeTimers;
export const useRealTimers = ht.useRealTimers;
export const advanceTimersByTime = ht.advanceTimersByTime;
export const runAllTimers = ht.runAllTimers;
export const getTimerCount = ht.getTimerCount;
export const advanceTimersToNextTimer = ht.advanceTimersToNextTimer;
