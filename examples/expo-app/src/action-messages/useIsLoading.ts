// Simplified useIsLoading — real version checks Redux loading tags
export function useIsLoading(_tag: string): { isLoading: boolean } {
  throw new Error('useIsLoading: must be mocked in tests');
}
