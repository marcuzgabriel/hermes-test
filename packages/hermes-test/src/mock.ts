// useMock — patches module exports for test mocking
// Works by replacing the getter functions on ESM namespace objects
//
// mockModule — jest.mock() equivalent: registers factory in global __HT_mocks
// so that externalized modules resolve through our mock layer at require() time.

import { spy, type Spy } from './spy';

type SavedDescriptor = { target: any; key: string; desc: PropertyDescriptor };
let savedDescriptors: SavedDescriptor[] = [];

// --- mockModule: jest.mock() equivalent ---
// Registers a mock factory for a module path. The bundler externalizes these
// modules and the require shim reads from this registry.
const mockRegistry: Record<string, Record<string, any>> = (globalThis as any).__HT_mocks || {};
(globalThis as any).__HT_mocks = mockRegistry;

export function mockModule(
  modulePath: string,
  factory: () => Record<string, any>
): void {
  const impl = factory();
  const wrapped = wrapWithSpies(impl);
  // The entry file pre-creates registry objects so __require returns them
  // before mockModule runs. Copy spy properties onto the existing object
  // so the reference stays the same.
  const existing = mockRegistry[modulePath];
  if (existing) {
    for (const key of Object.keys(wrapped)) {
      existing[key] = wrapped[key];
    }
  } else {
    mockRegistry[modulePath] = wrapped;
  }
}

function wrapWithSpies<T extends Record<string, any>>(impl: T): T {
  const wrapped: any = {};
  for (const key of Object.keys(impl)) {
    const value = impl[key];
    if (typeof value === 'function' && !(value as any)._isSpy) {
      wrapped[key] = spy(value);
    } else {
      wrapped[key] = value;
    }
  }
  return wrapped;
}

export function useMock<T extends Record<string, any>>(
  moduleExports: T,
  implementation: Partial<T>
): { [K in keyof T]: T[K] extends (...args: any[]) => any ? Spy<T[K]> : T[K] } {
  const wrapped = wrapWithSpies(implementation as Record<string, any>);

  for (const key of Object.keys(wrapped)) {
    // Save original descriptor for reset
    const desc = Object.getOwnPropertyDescriptor(moduleExports, key);
    if (desc) {
      savedDescriptors.push({ target: moduleExports, key, desc });
    }

    // Replace with a getter that returns our mock
    const mockValue = wrapped[key];
    try {
      Object.defineProperty(moduleExports, key, {
        get: () => mockValue,
        configurable: true,
        enumerable: true,
      });
    } catch {
      // If defineProperty fails, try direct assignment as fallback
      try {
        (moduleExports as any)[key] = mockValue;
      } catch {
        // Module is fully frozen — warn but continue
      }
    }
  }

  return wrapped as any;
}

export function resetMocks(): void {
  for (const { target, key, desc } of savedDescriptors) {
    try {
      Object.defineProperty(target, key, desc);
    } catch {
      // best effort
    }
  }
  savedDescriptors = [];
}
