declare global {
  interface HtMockFetch {
    (...handlers: import('hermes-test').FetchHandler[]): void;
    overwrite(...handlers: import('hermes-test').FetchHandler[]): void;
    reset(): void;
    clear(): void;
  }

  interface HtMock {
    (modulePath: string, factory: () => unknown): void;
    fetch: HtMockFetch;
  }

  /** Shallow render: auto-mock child components so the parent renders without deep dependencies. */
  function htShallow(componentPath: string): void;

  /** Remove a previously registered mock so the real module is used. No-op at runtime (bundler directive). */
  function htUnmock(modulePath: string): void;

  const ht: { mock: HtMock; shallow: typeof htShallow; unmock: typeof htUnmock };
}
export {};
