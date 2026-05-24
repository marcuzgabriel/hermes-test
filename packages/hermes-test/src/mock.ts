// useMock — patches module exports for test mocking
// Works by replacing the getter functions on ESM namespace objects
//
// mockModule — jest.mock() equivalent: registers factory in global __HT_mocks
// so that externalized modules resolve through our mock layer at require() time.

import { spy, type Spy } from './spy';

type SavedDescriptor = { target: any; key: string; desc: PropertyDescriptor };
let savedDescriptors: SavedDescriptor[] = [];

// --- mockModule: jest.mock() equivalent ---
// Registers a mock factory for a module path, scoped to the current test file.
// The bundler wraps mocked module exports in Proxies that check the per-file
// mock registry at access time. This allows multiple files to mock the same
// module with different implementations in a single bundle.
const mockRegistry: Record<string, Record<string, any>> = (globalThis as any).__HT_mocks || {};
(globalThis as any).__HT_mocks = mockRegistry;

// Per-file mock scoping: __HT_file_mocks[filename][modulePath] = mock
const fileMocks: Record<string, Record<string, any>> =
  (globalThis as any).__HT_file_mocks || ((globalThis as any).__HT_file_mocks = {});

// Track patches applied by mockModule so they can be undone between files
let mockModulePatches: { target: any; key: string; original: any }[] = [];

export function mockModule(
  modulePath: string,
  factory: () => Record<string, any>
): void {
  const impl = factory();
  const value = typeof impl === 'function' ? impl : wrapWithSpies(impl);

  // Register in per-file scope only — no global registry pollution
  const currentFile = (globalThis as any).__currentTestFile || '__global__';
  if (!fileMocks[currentFile]) fileMocks[currentFile] = {};
  fileMocks[currentFile][modulePath] = value;

  // Also patch the real module's properties so module-level destructuring picks up
  // the mock. Example: `const { fn } = api.endpoints` captures at init time — the
  // Proxy can't intercept it. But if we overwrite `api.endpoints.fn` on the real
  // object, the destructured `fn` still points to the same object's property.
  // This covers the case where a hook destructures `{ endpoints }` from a Proxy:
  //   const { method } = endpoints;  // captured at init
  // If we patch `endpoints.method = spy`, the captured `method` is updated.
  //
  // We look up the real module via __HT_mocks (for externalized) or by requiring
  // through the shadow wrapper's _getReal().
  const globalMock = mockRegistry[modulePath];
  if (globalMock && typeof globalMock === 'object' && typeof value === 'object') {
    for (const key of Object.keys(value)) {
      if (key === 'default' || key === '__esModule') continue;
      try {
        if (key in globalMock) {
          mockModulePatches.push({ target: globalMock, key, original: globalMock[key] });
          globalMock[key] = value[key];
        }
      } catch { /* frozen or non-configurable */ }
    }
    // Also patch 'default' export if mock provides it
    if ('default' in value && 'default' in globalMock) {
      const mockDefault = value['default'];
      const realDefault = globalMock['default'];
      if (realDefault && typeof realDefault === 'object' && typeof mockDefault === 'object') {
        for (const key of Object.keys(mockDefault)) {
          try {
            if (key in realDefault) {
              mockModulePatches.push({ target: realDefault, key, original: realDefault[key] });
              realDefault[key] = mockDefault[key];
            }
          } catch { /* frozen */ }
        }
      }
    }
  }
}

// Called between test files to undo mockModule patches
export function resetMockModulePatches(): void {
  for (const { target, key, original } of mockModulePatches) {
    try { target[key] = original; } catch { /* best effort */ }
  }
  mockModulePatches = [];
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
