// Simplified useErrorHandling — real version wraps Redux dispatch + error state
export function useErrorHandling(_tag: string): {
  error: string | null;
  dispatchWithErrorHandler: (action: any) => Promise<any>;
  removeError: (tag: string) => void;
} {
  throw new Error('useErrorHandling: must be mocked in tests');
}
