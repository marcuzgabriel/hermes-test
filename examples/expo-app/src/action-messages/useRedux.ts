// Simplified useAppSelector — in real app this wraps react-redux useSelector
export function useAppSelector<T>(selector: (state: any) => T): T {
  // Real implementation would use React-Redux.
  // Tests mock this entire module export.
  throw new Error('useAppSelector: must be mocked in tests');
}
