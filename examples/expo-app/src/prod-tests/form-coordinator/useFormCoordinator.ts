// Real hook — form state management with Redux backing
import { useCallback, useMemo, useState } from 'react';
import { useSelector } from 'react-redux';

export const useFormCoordinator = (formId: string) => {
  const formState = useSelector((s: any) => s.form?.[formId] ?? {});
  const [localValues, setLocalValues] = useState<Record<string, any>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  const getValue = useCallback((key: string) => localValues[key] ?? formState[key] ?? '', [localValues, formState]);

  const updateValue = useCallback((key: string, value: any) => {
    setLocalValues(prev => ({ ...prev, [key]: value }));
    setErrors(prev => { const next = { ...prev }; delete next[key]; return next; });
  }, []);

  const saveValues = useCallback(() => {
    return { formId, values: { ...formState, ...localValues } };
  }, [formId, formState, localValues]);

  const resetForm = useCallback(() => {
    setLocalValues({});
    setErrors({});
  }, []);

  const hasInputErrors = useCallback((requiredFields: string[]) => {
    const newErrors: Record<string, string> = {};
    for (const field of requiredFields) {
      if (!getValue(field)) newErrors[field] = 'required';
    }
    setErrors(newErrors);
    return Object.keys(newErrors).length > 0;
  }, [getValue]);

  return { getValue, updateValue, saveValues, resetForm, hasInputErrors, errors, localValues };
};
