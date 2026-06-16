declare global {
  interface HtMockFetch {
    (...handlers: import('hermes-test').FetchHandler[]): void
    overwrite(...handlers: import('hermes-test').FetchHandler[]): void
    reset(): void
    clear(): void
  }

  interface HtMock {
    (modulePath: string, factory: () => Record<string, unknown>): void
    fetch: HtMockFetch
  }

  /** Shallow render: auto-mock child components so the parent renders without deep dependencies. */
  function htShallow(componentPath: string): void

  const ht: { mock: HtMock; shallow: typeof htShallow }
}
export {}
