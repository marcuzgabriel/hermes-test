// Global type declarations for hermes-test
// ht is available globally without import, like jest.

interface HtFetchHandler {
  method: string;
  url: string | RegExp;
  handler: (req: any) => any;
  once?: boolean;
}

interface HtMockFetch {
  (...handlers: HtFetchHandler[]): void;
  overwrite(...handlers: HtFetchHandler[]): void;
  reset(): void;
  clear(): void;
}

interface HtMock {
  (modulePath: string, factory: () => Record<string, unknown>): void;
  fetch: HtMockFetch;
}

interface Ht {
  mock: HtMock;
}

declare const ht: Ht;
