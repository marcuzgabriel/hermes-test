// Type declarations for hermes-test

export type Spy<F extends (...args: any[]) => any = (...args: any[]) => any> =
  F & {
    readonly calls: ReadonlyArray<Parameters<F>>;
    readonly callCount: number;
    readonly returnValues: ReadonlyArray<ReturnType<F>>;
    reset(): void;
    setImpl(impl: F): void;
    returns(value: ReturnType<F>): Spy<F>;
    mockImplementation(fn: F): Spy<F>;
    mockImplementationOnce(fn: F): Spy<F>;
    mockReturnValue(value: ReturnType<F>): Spy<F>;
    mockReturnValueOnce(value: ReturnType<F>): Spy<F>;
    mockResolvedValue(value: any): Spy<F>;
    mockResolvedValueOnce(value: any): Spy<F>;
    mockRejectedValue(value: any): Spy<F>;
    mockRejectedValueOnce(value: any): Spy<F>;
    mockClear(): void;
    mockReset(): void;
    mockRestore(): void;
    _isSpy: true;
  };

export interface Assertion {
  toBe(expected: any): void;
  toEqual(expected: any): void;
  toContain(item: any): void;
  toMatch(pattern: string | RegExp): void;
  toThrow(message?: string | RegExp): void;
  toBeNull(): void;
  toBeUndefined(): void;
  toBeDefined(): void;
  toBeTruthy(): void;
  toBeFalsy(): void;
  toBeGreaterThan(n: number): void;
  toBeGreaterThanOrEqual(n: number): void;
  toBeLessThan(n: number): void;
  toBeLessThanOrEqual(n: number): void;
  toBeInstanceOf(cls: any): void;
  toHaveLength(n: number): void;
  toHaveProperty(key: string, value?: any): void;
  wasCalled(): void;
  wasCalledOnce(): void;
  wasCalledTimes(n: number): void;
  wasCalledWith(...args: any[]): void;
  wasLastCalledWith(...args: any[]): void;
  wasNeverCalled(): void;
  toHaveBeenCalled(): void;
  toHaveBeenCalledTimes(n: number): void;
  toHaveBeenCalledWith(...args: any[]): void;
  toHaveBeenLastCalledWith(...args: any[]): void;
  not: Assertion;
}

export interface HookResult<T> {
  readonly result: { readonly current: T };
  readonly current: T;
  readonly history: ReadonlyArray<T>;
  readonly renderCount: number;
  rerender(props?: any): void;
  unmount(): void;
}

export function test(name: string, fn: (() => void | Promise<void>) | ((ctx: any) => void | Promise<void>), options?: { timeout?: number; skip?: boolean; only?: boolean }): void;
export function group(name: string, fn: () => void): void;
export function expect(actual: any): Assertion;
export function spy<F extends (...args: any[]) => any = () => void>(impl?: F): Spy<F>;
export function spyOn<T extends object, K extends keyof T>(obj: T, method: K): Spy;
export function beforeEach(fn: () => void | Promise<void>): void;
export function afterEach(fn: () => void | Promise<void>): void;
export function beforeAll(fn: () => void | Promise<void>): void;
export function afterAll(fn: () => void | Promise<void>): void;
export function renderHook<T>(hookFn: (props?: any) => T, options?: { initialProps?: any; wrapper?: any }): HookResult<T>;
export function act(fn: () => void | Promise<void>): void;
export function waitFor<T>(predicate: () => T | false | null | undefined, options?: { timeout?: number; interval?: number }): T;
export function mockModule(modulePath: string, factory: () => Record<string, any>): void;
export function useMock<T extends Record<string, any>>(moduleExports: T, implementation: Partial<T>): any;
export function mockFetch(...handlers: any[]): void;
export function mockFetchUse(...handlers: any[]): void;
export function mockFetchReset(): void;
export function mockFetchClear(): void;
export function flushAsync<T = any>(promise: Promise<T> | T): T;
export function useFakeTimers(): void;
export function useRealTimers(): void;
export function advanceTimersByTime(ms: number): void;
export function runAllTimers(): void;
export function getTimerCount(): number;
export function advanceTimersToNextTimer(): void;

export const http: {
  get(url: string, handler: (req: any) => any): any;
  post(url: string, handler: (req: any) => any): any;
  put(url: string, handler: (req: any) => any): any;
  patch(url: string, handler: (req: any) => any): any;
  delete(url: string, handler: (req: any) => any): any;
};

export const HttpResponse: {
  json(body: any, init?: { status?: number; headers?: Record<string, string> }): any;
  text(body: string, init?: { status?: number }): any;
  error(): any;
};
