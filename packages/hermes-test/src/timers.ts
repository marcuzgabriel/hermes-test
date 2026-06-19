// Fake timer control for deterministic testing.
//
// Usage:
//   useFakeTimers();
//   setTimeout(fn, 1000);
//   advanceTimersByTime(1000); // fn fires synchronously
//   useRealTimers();

// Save the real Date.now function REFERENCE before useFakeTimers can overwrite it.
// useFakeTimers does `globalThis.Date.now = () => fakeNow` which mutates the original
// Date object's .now property. So we must save the function itself, not the constructor.
const _savedDateNow: () => number = Date.now;
export function realDateNow(): number {
  return _savedDateNow();
}

interface PendingTimer {
  id: number;
  fn: () => void;
  delay: number;
  fireAt: number;
  type: 'timeout' | 'interval';
  interval: number;
}

let fakeNow = 0;
let nextId = 1;
let pending: PendingTimer[] = [];
let isFake = false;

// Save originals
const _setTimeout = (globalThis as any).setTimeout;
const _clearTimeout = (globalThis as any).clearTimeout;
const _setInterval = (globalThis as any).setInterval;
const _clearInterval = (globalThis as any).clearInterval;
const _Date = globalThis.Date;
const _drain = (globalThis as any).__HT_drain;

function runTimerCallback(fn: () => void) {
  const result = fn();
  if (result && typeof (result as any).then === 'function' && typeof _drain === 'function') {
    _drain();
  }
}

function fakeSetTimeout(fn: () => void, delay: number = 0): number {
  const id = nextId++;
  pending.push({ id, fn, delay, fireAt: fakeNow + delay, type: 'timeout', interval: 0 });
  return id;
}

function fakeClearTimeout(id: number) {
  pending = pending.filter(t => t.id !== id);
}

function fakeSetInterval(fn: () => void, delay: number): number {
  const id = nextId++;
  pending.push({ id, fn, delay, fireAt: fakeNow + delay, type: 'interval', interval: delay });
  return id;
}

function fakeClearInterval(id: number) {
  pending = pending.filter(t => t.id !== id);
}

export function useFakeTimers(initialTime?: number) {
  fakeNow = initialTime ?? 0;
  nextId = 1;
  pending = [];
  isFake = true;

  (globalThis as any).setTimeout = fakeSetTimeout;
  (globalThis as any).clearTimeout = fakeClearTimeout;
  (globalThis as any).setInterval = fakeSetInterval;
  (globalThis as any).clearInterval = fakeClearInterval;

  // Override Date.now()
  (globalThis as any).Date = function (...args: any[]) {
    if (args.length === 0) return new _Date(fakeNow);
    return new (_Date as any)(...args);
  } as any;
  (globalThis as any).Date.now = () => fakeNow;
  (globalThis as any).Date.parse = _Date.parse;
  (globalThis as any).Date.UTC = _Date.UTC;
  // Copy prototype so instanceof checks and methods still work
  (globalThis as any).Date.prototype = _Date.prototype;
}

export function useRealTimers() {
  isFake = false;
  pending = [];
  (globalThis as any).setTimeout = _setTimeout;
  (globalThis as any).clearTimeout = _clearTimeout;
  (globalThis as any).setInterval = _setInterval;
  (globalThis as any).clearInterval = _clearInterval;
  (globalThis as any).Date = _Date;
}

export function advanceTimersByTime(ms: number) {
  if (!isFake) throw new Error('advanceTimersByTime called without useFakeTimers()');
  const target = fakeNow + ms;

  while (fakeNow < target) {
    // Find next timer to fire
    const ready = pending.filter(t => t.fireAt <= target).sort((a, b) => a.fireAt - b.fireAt);
    if (ready.length === 0) {
      fakeNow = target;
      break;
    }

    const timer = ready[0];
    fakeNow = timer.fireAt;

    if (timer.type === 'timeout') {
      pending = pending.filter(t => t.id !== timer.id);
      runTimerCallback(timer.fn);
    } else {
      // interval: fire and reschedule
      timer.fireAt += timer.interval;
      runTimerCallback(timer.fn);
    }
  }

  fakeNow = target;
}

export function runAllTimers() {
  if (!isFake) throw new Error('runAllTimers called without useFakeTimers()');
  let safety = 1000;
  while (pending.length > 0 && safety-- > 0) {
    const next = pending.reduce((min, t) => (t.fireAt < min.fireAt ? t : min));
    fakeNow = next.fireAt;
    if (next.type === 'timeout') {
      pending = pending.filter(t => t.id !== next.id);
      runTimerCallback(next.fn);
    } else {
      next.fireAt += next.interval;
      runTimerCallback(next.fn);
    }
  }
}

export function getTimerCount(): number {
  return pending.length;
}

export function advanceTimersToNextTimer() {
  if (!isFake || pending.length === 0) return;
  const next = pending.reduce((min, t) => (t.fireAt < min.fireAt ? t : min));
  advanceTimersByTime(next.fireAt - fakeNow);
}
