// Pattern: Complex form hook with validation and submit
// Demonstrates: renderHook, act, spy, expect.objectContaining
import { test, group, renderHook, act, spy, expect } from 'hermes-test';
import { useState, useCallback } from 'react';

// --- Hook under test ---
function useForm(initial: Record<string, string>) {
  const [fields, setFields] = useState(initial);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitted, setSubmitted] = useState(false);

  const setField = useCallback((name: string, value: string) => {
    setFields((f) => ({ ...f, [name]: value }));
    setErrors((e) => { const copy = { ...e }; delete copy[name]; return copy; });
  }, []);

  const validate = useCallback(() => {
    const errs: Record<string, string> = {};
    if (!fields.name) errs.name = 'Name is required';
    if (!fields.email?.includes('@')) errs.email = 'Invalid email';
    setErrors(errs);
    return Object.keys(errs).length === 0;
  }, [fields]);

  const submit = useCallback((onSubmit: (data: typeof fields) => void) => {
    if (validate()) { onSubmit(fields); setSubmitted(true); }
  }, [fields, validate]);

  return { fields, errors, submitted, setField, validate, submit };
}

// --- Tests ---
group('useForm', () => {
  test('initial state has fields and no errors', () => {
    const { current } = renderHook(() => useForm({ name: '', email: '' }));
    expect(current.fields).toEqual({ name: '', email: '' });
    expect(current.errors).toEqual({});
    expect(current.submitted).toBe(false);
  });

  test('setField updates value and clears error', () => {
    const result = renderHook(() => useForm({ name: '', email: '' }));
    act(() => result.current.validate()); // trigger errors
    expect(result.current.errors.name).toBe('Name is required');
    act(() => result.current.setField('name', 'Alice'));
    expect(result.current.fields.name).toBe('Alice');
    expect(result.current.errors.name).toBeUndefined();
  });

  test('validate catches missing fields', () => {
    const result = renderHook(() => useForm({ name: '', email: 'bad' }));
    act(() => result.current.validate());
    expect(result.current.errors).toEqual(
      expect.objectContaining({ name: 'Name is required', email: 'Invalid email' }));
  });

  test('submit calls handler when valid', () => {
    const onSubmit = spy((_data: any) => {});
    const result = renderHook(() => useForm({ name: 'Alice', email: 'a@b.com' }));
    act(() => result.current.submit(onSubmit));
    expect(onSubmit).wasCalledOnce();
    expect(onSubmit).wasCalledWith({ name: 'Alice', email: 'a@b.com' });
    expect(result.current.submitted).toBe(true);
  });

  test('submit blocks when invalid', () => {
    const onSubmit = spy((_data: any) => {});
    const result = renderHook(() => useForm({ name: '', email: '' }));
    act(() => result.current.submit(onSubmit));
    expect(onSubmit).wasCalledTimes(0);
    expect(result.current.submitted).toBe(false);
  });
});
