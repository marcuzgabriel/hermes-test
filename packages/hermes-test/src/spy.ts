export type Spy<F extends (...args: any[]) => any = (...args: any[]) => any> =
  F & {
    readonly calls: ReadonlyArray<Parameters<F>>;
    readonly callCount: number;
    readonly returnValues: ReadonlyArray<ReturnType<F>>;
    reset(): void;

    // Core
    setImpl(impl: F): void;
    returns(value: ReturnType<F>): Spy<F>;

    // Jest-compatible API
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

    // spyOn support
    _restore?: () => void;
    _isSpy: true;
  };

// Registry of all spies created — clearAllMocks() clears them all at once.
const _allSpies: Spy[] = [];

export function clearAllMocks(): void {
  for (const s of _allSpies) s.mockClear();
}

export function spy<F extends (...args: any[]) => any = () => void>(
  impl?: F
): Spy<F> {
  let baseImpl: F | undefined = impl;
  const onceImpls: F[] = []; // FIFO queue for mockImplementationOnce/mockReturnValueOnce
  const calls: any[][] = [];
  const returnValues: any[] = [];

  const fn = function (this: any, ...args: any[]) {
    calls.push(args);

    // Once-implementations take priority (FIFO)
    let ret: any;
    if (onceImpls.length > 0) {
      const onceFn = onceImpls.shift()!;
      ret = onceFn.apply(this, args);
    } else {
      ret = baseImpl ? baseImpl.apply(this, args) : undefined;
    }

    returnValues.push(ret);
    return ret;
  } as unknown as Spy<F>;

  // Preserve prototype chain so `new spy(...)` creates instances with the
  // original class's prototype methods (e.g. class methods like write()).
  if (impl && impl.prototype) {
    fn.prototype = impl.prototype;
  }

  Object.defineProperties(fn, {
    calls: { get: () => calls },
    callCount: { get: () => calls.length },
    returnValues: { get: () => returnValues },
    _isSpy: { value: true },
  });

  // --- Core API ---

  (fn as any).reset = () => {
    calls.length = 0;
    returnValues.length = 0;
    onceImpls.length = 0;
  };

  (fn as any).setImpl = (newImpl: F) => {
    baseImpl = newImpl;
    return fn;
  };

  (fn as any).returns = (value: any) => {
    baseImpl = (() => value) as any;
    return fn;
  };

  // --- Jest-compatible API ---

  (fn as any).mockImplementation = (newImpl: F) => {
    baseImpl = newImpl;
    return fn;
  };

  (fn as any).mockImplementationOnce = (onceFn: F) => {
    onceImpls.push(onceFn);
    return fn;
  };

  (fn as any).mockReturnValue = (value: any) => {
    baseImpl = (() => value) as any;
    return fn;
  };

  (fn as any).mockReturnValueOnce = (value: any) => {
    onceImpls.push((() => value) as any);
    return fn;
  };

  (fn as any).mockResolvedValue = (value: any) => {
    baseImpl = (() => Promise.resolve(value)) as any;
    return fn;
  };

  (fn as any).mockResolvedValueOnce = (value: any) => {
    onceImpls.push((() => Promise.resolve(value)) as any);
    return fn;
  };

  (fn as any).mockRejectedValue = (value: any) => {
    baseImpl = (() => Promise.reject(value)) as any;
    return fn;
  };

  (fn as any).mockRejectedValueOnce = (value: any) => {
    onceImpls.push((() => Promise.reject(value)) as any);
    return fn;
  };

  (fn as any).mockClear = () => {
    calls.length = 0;
    returnValues.length = 0;
    onceImpls.length = 0;
  };

  (fn as any).mockReset = () => {
    calls.length = 0;
    returnValues.length = 0;
    onceImpls.length = 0;
    baseImpl = undefined;
  };

  (fn as any).mockRestore = () => {
    (fn as any).mockReset();
    if ((fn as any)._restore) (fn as any)._restore();
  };

  _allSpies.push(fn);
  return fn;
}

// --- spyOn: replace a method on an object with a spy, preserving original ---
export function spyOn<T extends Record<string, any>>(
  obj: T,
  method: keyof T & string,
): Spy {
  const original = obj[method];
  const s = spy(typeof original === 'function' ? original.bind(obj) : undefined);
  (s as any)._restore = () => { obj[method] = original; };
  obj[method] = s as any;
  return s;
}
