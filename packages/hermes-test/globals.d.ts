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

  const ht: { mock: HtMock }
}
export {}
