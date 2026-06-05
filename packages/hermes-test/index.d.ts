// Type declarations for hermes-test

// --- Spy ---

export type Spy<F extends (...args: any[]) => any = (...args: any[]) => any> =
  F & {
    readonly calls: ReadonlyArray<Parameters<F>>;
    readonly callCount: number;
    readonly returnValues: ReadonlyArray<ReturnType<F>>;
    reset(): void;
    setImpl(impl: F): Spy<F>;
    returns(value: ReturnType<F>): Spy<F>;
    mockImplementation(fn: F): Spy<F>;
    mockImplementationOnce(fn: F): Spy<F>;
    mockReturnValue(value: ReturnType<F>): Spy<F>;
    mockReturnValueOnce(value: ReturnType<F>): Spy<F>;
    mockResolvedValue<V>(value: V): Spy<(...args: Parameters<F>) => Promise<V>>;
    mockResolvedValueOnce<V>(value: V): Spy<F>;
    mockRejectedValue(value: unknown): Spy<F>;
    mockRejectedValueOnce(value: unknown): Spy<F>;
    mockClear(): void;
    mockReset(): void;
    mockRestore(): void;
    readonly _isSpy: true;
  };

export function spy(): Spy<(...args: any[]) => undefined>;
export function spy<F extends (...args: any[]) => any>(impl: F): Spy<F>;
export function spyOn<T extends Record<string, any>, K extends keyof T & string>(
  obj: T,
  method: K,
): T[K] extends (...args: any[]) => any ? Spy<T[K]> : Spy;
export function clearAllMocks(): void;

// --- Expect ---

export interface AsymmetricMatcher {
  readonly __htMatcher: true;
  matches(value: unknown): boolean;
}

export interface Assertion<T = unknown> {
  toBe(expected: T): void;
  toEqual(expected: T): void;
  toContain(item: T extends ReadonlyArray<infer U> ? U : T extends string ? string : unknown): void;
  toContainEqual(item: T extends ReadonlyArray<infer U> ? U : unknown): void;
  toMatch(pattern: string | RegExp): void;
  toThrow(message?: string | RegExp): void;
  toBeNull(): void;
  toBeUndefined(): void;
  toBeDefined(): void;
  toBeTruthy(): void;
  toBeFalsy(): void;
  toBeGreaterThan(n: number): void;
  toBeLessThan(n: number): void;
  toBeInstanceOf(cls: new (...args: any[]) => any): void;
  toHaveLength(n: number): void;
  toBeCloseTo(expected: number, precision?: number): void;

  // Spy-specific
  wasCalled(): void;
  wasCalledOnce(): void;
  wasCalledTimes(n: number): void;
  wasCalledWith(...args: any[]): void;
  wasLastCalledWith(...args: any[]): void;
  wasNeverCalled(): void;

  // Jest-compatible aliases
  toHaveBeenCalled(): void;
  toHaveBeenCalledTimes(n: number): void;
  toHaveBeenCalledWith(...args: any[]): void;
  toHaveBeenLastCalledWith(...args: any[]): void;

  // Element matchers (for render() results)
  toBeRendered(): void;
  toHaveTextContent(expected: string | RegExp): void;
  toContainElement(child: unknown): void;
  toBeEmpty(): void;
  toHaveDisplayValue(expected: string | RegExp): void;
  toHaveProp(name: string, value?: unknown): void;
  toHaveStyle(expected: Record<string, unknown>): void;
  toBeEnabled(): void;
  toBeDisabled(): void;
  toBeVisible(): void;

  not: Assertion<T>;

  resolves: {
    toBe(expected: unknown): Promise<void>;
    toEqual(expected: unknown): Promise<void>;
    toBeDefined(): Promise<void>;
    toBeUndefined(): Promise<void>;
    toBeNull(): Promise<void>;
    toBeTruthy(): Promise<void>;
    toBeFalsy(): Promise<void>;
  };

  rejects: {
    toThrow(message?: string | RegExp): Promise<void>;
  };
}

export interface ExpectFunction {
  <T>(actual: T): Assertion<T>;
  anything(): AsymmetricMatcher;
  any(constructor: new (...args: any[]) => any): AsymmetricMatcher;
  objectContaining<T extends Record<string, unknown>>(subset: T): AsymmetricMatcher;
  arrayContaining<T>(arr: T[]): AsymmetricMatcher;
  stringContaining(substr: string): AsymmetricMatcher;
  stringMatching(pattern: string | RegExp): AsymmetricMatcher;
}

export const expect: ExpectFunction;

// --- Test structure ---

export interface TestOptions {
  timeout?: number;
  skip?: boolean;
  only?: boolean;
}

export interface TestFunction {
  (name: string, fn: () => void | Promise<void>, options?: TestOptions): void;
  skip(name: string, fn: () => void | Promise<void>): void;
  only(name: string, fn: () => void | Promise<void>): void;
}

export const test: TestFunction;
export function group(name: string, fn: () => void): void;
export function beforeEach(fn: () => void | Promise<void>): void;
export function afterEach(fn: () => void | Promise<void>): void;
export function beforeAll(fn: () => void | Promise<void>): void;
export function afterAll(fn: () => void | Promise<void>): void;

// --- Hooks ---

export interface HookResult<T> {
  readonly result: { readonly current: T };
  readonly current: T;
  readonly history: ReadonlyArray<T>;
  readonly renderCount: number;
  rerender(props?: unknown): void;
  unmount(): void;
}

export interface RenderHookOptions<P = unknown> {
  initialProps?: P;
  wrapper?: React.ComponentType<{ children: React.ReactNode }>;
}

export function renderHook<T, P = unknown>(
  hookFn: (props?: P) => T,
  options?: RenderHookOptions<P>,
): HookResult<T>;

export function act(fn: () => void | Promise<void>): void;

export function waitFor<T>(
  predicate: () => T | false | null | undefined,
  options?: { timeout?: number; interval?: number },
): T;

export function flushAsync<T>(promise: Promise<T>): T;
export function flushAsync<T>(value: T): T;

// --- Component rendering ---

export interface HTNode {
  type: string;
  props: Record<string, any>;
  children: HTNode[];
  text?: string;
}

export interface RenderResult {
  container: HTNode;
  getByText(text: string | RegExp): HTNode;
  getAllByText(text: string | RegExp): HTNode[];
  queryByText(text: string | RegExp): HTNode | null;
  queryAllByText(text: string | RegExp): HTNode[];
  getByTestId(testID: string | RegExp): HTNode;
  getAllByTestId(testID: string | RegExp): HTNode[];
  queryByTestId(testID: string | RegExp): HTNode | null;
  queryAllByTestId(testID: string | RegExp): HTNode[];
  getByProps(props: Record<string, any>): HTNode;
  getAllByProps(props: Record<string, any>): HTNode[];
  queryByProps(props: Record<string, any>): HTNode | null;
  queryAllByProps(props: Record<string, any>): HTNode[];
  getByType(type: string): HTNode;
  getAllByType(type: string): HTNode[];
  queryByType(type: string): HTNode | null;
  queryAllByType(type: string): HTNode[];
  toJSON(): any;
  toTree(): string;
  rerender(element: React.ReactElement): void;
  unmount(): void;
}

export function render(element: React.ReactElement): RenderResult;

export interface FireEventObject {
  (node: HTNode, eventName: string, ...args: any[]): void;
  press(node: HTNode, event?: any): void;
  changeText(node: HTNode, text: string): void;
  scroll(node: HTNode, event: any): void;
}

export const fireEvent: FireEventObject;

// --- Mocking ---

export function mockModule(modulePath: string, factory: () => Record<string, unknown>): void;
export function useMock<T extends Record<string, unknown>>(
  moduleExports: T,
  implementation: Partial<{ [K in keyof T]: T[K] extends (...args: any[]) => any ? Spy<T[K]> | T[K] : T[K] }>,
): void;

// --- Fetch mocking ---

export interface MockRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: unknown;
}

export interface MockResponse {
  body?: unknown;
  status?: number;
  statusText?: string;
  headers?: Record<string, string>;
}

export type FetchHandler = {
  method: string;
  url: string | RegExp;
  handler: (req: MockRequest) => MockResponse;
  once?: boolean;
};

export function mockFetch(...handlers: FetchHandler[]): void;
export function mockFetchUse(...handlers: FetchHandler[]): void;
export function mockFetchReset(): void;
export function mockFetchClear(): void;

export const http: {
  get(url: string | RegExp, handler: (req: MockRequest) => MockResponse): FetchHandler;
  post(url: string | RegExp, handler: (req: MockRequest) => MockResponse): FetchHandler;
  put(url: string | RegExp, handler: (req: MockRequest) => MockResponse): FetchHandler;
  patch(url: string | RegExp, handler: (req: MockRequest) => MockResponse): FetchHandler;
  delete(url: string | RegExp, handler: (req: MockRequest) => MockResponse): FetchHandler;
};

export const HttpResponse: {
  json<T>(body: T, init?: { status?: number; headers?: Record<string, string> }): MockResponse;
  text(body: string, init?: { status?: number }): MockResponse;
  error(): MockResponse;
};

// --- Redux store ---

export interface StoreContext {
  readonly store: any;
  dispatch(action: any): any;
  getState(): any;
  setState(state: Record<string, any>): void;
  patchState(partial: Record<string, any>): void;
  renderHookWithReduxStore<T>(hookFn: (props?: any) => T, options?: { initialProps?: any }): HookResult<T>;
}

export function withStore(initialState?: Record<string, any>): StoreContext;
export function withAppReducer(reducer: (state: any, action: any) => any, preloadedState?: Record<string, any>): StoreContext;

// --- Timers ---

export function useFakeTimers(initialTime?: number): void;
export function useRealTimers(): void;
export function advanceTimersByTime(ms: number): void;
export function runAllTimers(): void;
export function getTimerCount(): number;
export function advanceTimersToNextTimer(): void;

// --- React import (for type inference) ---

// React types used if @types/react is installed
import type React from 'react';
