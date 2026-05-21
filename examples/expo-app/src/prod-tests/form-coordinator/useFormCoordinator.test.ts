import { test, group, beforeEach, act, expect } from 'hermes-test';
// Production test port: useFormCoordinator — 8 tests
import { withStore } from '../shared/testStore';
import { useFormCoordinator } from './useFormCoordinator';

let ctx: ReturnType<typeof withStore>;
beforeEach(() => { ctx = withStore({ form: { myForm: { name: 'prefilled', email: '' } } }); });

group('useFormCoordinator', () => {
  test('getValue returns prefilled value from store', () => {
    const { current } = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    expect(current.getValue('name')).toBe('prefilled');
  });

  test('getValue returns empty for missing keys', () => {
    const { current } = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    expect(current.getValue('phone')).toBe('');
  });

  test('updateValue changes local state', () => {
    const hook = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    act(() => { hook.current.updateValue('name', 'updated'); });
    expect(hook.current.getValue('name')).toBe('updated');
  });

  test('saveValues merges store + local', () => {
    const hook = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    act(() => { hook.current.updateValue('phone', '12345678'); });
    const saved = hook.current.saveValues();
    expect(saved.formId).toBe('myForm');
    expect(saved.values.name).toBe('prefilled');
    expect(saved.values.phone).toBe('12345678');
  });

  test('resetForm clears local values and errors', () => {
    const hook = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    act(() => { hook.current.updateValue('name', 'changed'); });
    act(() => { hook.current.resetForm(); });
    expect(hook.current.getValue('name')).toBe('prefilled'); // falls back to store
    expect(hook.current.errors).toEqual({});
  });

  test('hasInputErrors returns true for empty required fields', () => {
    const hook = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    let hasErrors: boolean;
    act(() => { hasErrors = hook.current.hasInputErrors(['email', 'phone']); });
    expect(hook.current.errors.email).toBe('required');
    expect(hook.current.errors.phone).toBe('required');
  });

  test('hasInputErrors returns false when fields filled', () => {
    const hook = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    act(() => { hook.current.updateValue('email', 'test@test.dk'); });
    let hasErrors: boolean;
    act(() => { hasErrors = hook.current.hasInputErrors(['name', 'email']); });
    expect(Object.keys(hook.current.errors).length).toBe(0);
  });

  test('updateValue clears error for that field', () => {
    const hook = ctx.renderHookWithReduxStore(() => useFormCoordinator('myForm'));
    act(() => { hook.current.hasInputErrors(['email']); });
    expect(hook.current.errors.email).toBe('required');
    act(() => { hook.current.updateValue('email', 'filled'); });
    expect(hook.current.errors.email).toBeUndefined();
  });
});
