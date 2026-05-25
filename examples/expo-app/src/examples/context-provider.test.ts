// React Context — testing hooks that depend on Provider context
import { test, expect, group, renderHook, act, spy } from 'hermes-test';

const React = require('react');

// --- Context + Provider ---

type Theme = 'light' | 'dark';
type ThemeContextValue = { theme: Theme; toggle: () => void };

const ThemeContext = React.createContext<ThemeContextValue | null>(null);

function ThemeProvider({ children, initial = 'light' as Theme }: { children: React.ReactNode; initial?: Theme }) {
  const [theme, setTheme] = React.useState<Theme>(initial);
  const toggle = React.useCallback(() => setTheme((t: Theme) => t === 'light' ? 'dark' : 'light'), []);
  return React.createElement(ThemeContext.Provider, { value: { theme, toggle } }, children);
}

// --- Hook under test ---

function useTheme() {
  const ctx = React.useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be wrapped in ThemeProvider');
  return ctx;
}

function useThemedStyle() {
  const { theme } = useTheme();
  return {
    backgroundColor: theme === 'dark' ? '#000' : '#fff',
    color: theme === 'dark' ? '#fff' : '#000',
  };
}

// --- Tests ---

group('useContext with Provider wrapper', () => {
  const wrapper = ({ children }: { children: React.ReactNode }) =>
    React.createElement(ThemeProvider, null, children);

  const darkWrapper = ({ children }: { children: React.ReactNode }) =>
    React.createElement(ThemeProvider, { initial: 'dark' }, children);

  test('reads default theme from context', () => {
    const { result } = renderHook(() => useTheme(), { wrapper });
    expect(result.current.theme).toBe('light');
  });

  test('reads initial dark theme', () => {
    const { result } = renderHook(() => useTheme(), { wrapper: darkWrapper });
    expect(result.current.theme).toBe('dark');
  });

  test('toggle switches theme', () => {
    const { result } = renderHook(() => useTheme(), { wrapper });
    expect(result.current.theme).toBe('light');

    act(() => result.current.toggle());
    expect(result.current.theme).toBe('dark');

    act(() => result.current.toggle());
    expect(result.current.theme).toBe('light');
  });

  test('derived hook computes styles from context', () => {
    const { result } = renderHook(() => useThemedStyle(), { wrapper });
    expect(result.current.backgroundColor).toBe('#fff');
    expect(result.current.color).toBe('#000');
  });

  test('throws without provider', () => {
    expect(() => renderHook(() => useTheme())).toThrow('useTheme must be wrapped in ThemeProvider');
  });
});
