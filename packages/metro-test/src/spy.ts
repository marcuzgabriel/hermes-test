export type Spy<F extends (...args: any[]) => any = (...args: any[]) => any> =
  F & {
    readonly calls: ReadonlyArray<Parameters<F>>;
    readonly callCount: number;
    readonly returnValues: ReadonlyArray<ReturnType<F>>;
    reset(): void;
    setImpl(impl: F): void;
    returns(value: ReturnType<F>): Spy<F>;
    _isSpy: true;
  };

export function spy<F extends (...args: any[]) => any = () => void>(
  impl?: F
): Spy<F> {
  let currentImpl: F | undefined = impl;
  const calls: any[][] = [];
  const returnValues: any[] = [];

  const fn = function (this: any, ...args: any[]) {
    calls.push(args);
    const ret = currentImpl ? currentImpl.apply(this, args) : undefined;
    returnValues.push(ret);
    return ret;
  } as unknown as Spy<F>;

  Object.defineProperties(fn, {
    calls: { get: () => calls },
    callCount: { get: () => calls.length },
    returnValues: { get: () => returnValues },
    _isSpy: { value: true },
  });

  (fn as any).reset = () => {
    calls.length = 0;
    returnValues.length = 0;
  };

  (fn as any).setImpl = (newImpl: F) => {
    currentImpl = newImpl;
  };

  (fn as any).returns = (value: any) => {
    currentImpl = (() => value) as any;
    return fn;
  };

  return fn;
}
