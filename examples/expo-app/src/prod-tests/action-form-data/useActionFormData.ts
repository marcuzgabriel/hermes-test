// Real production hook — faithful copy
// Only change: useSelector reads from generic store instead of monorepo's useAppSelector
import { useMemo } from 'react';
import { useSelector } from 'react-redux';

interface Return { actionConfig: any | null; actionData: any | null; }

export const useActionFormData = (
  actionGroupId: string,
  actionId: string,
  actionType: string,
  formType: string,
): Return => {
  const actionGroup = useSelector((state: any) => {
    const queryKey = `getActionGroup({"actionGroupId":"${actionGroupId}"})`;
    return state.api?.queries?.[queryKey]?.data;
  });

  const actionTypeConfig = useSelector((state: any) => {
    const keys = Object.keys(state.api?.queries ?? {});
    const queryKey = keys.find((k: string) => k.startsWith('getActionTypeConfig'));
    return queryKey ? state.api?.queries?.[queryKey]?.data : null;
  });

  const action = useMemo(() => {
    return actionGroup?.customerActions?.find((a: any) => a.actionId === actionId);
  }, [actionGroup, actionId]);

  const apiConfig = useMemo(() => {
    const configKey = `${actionType}::${formType}`;
    return actionTypeConfig?.actionConfigs?.find(
      (config: any) => `${config.actionType}::${config.formType}` === configKey,
    );
  }, [actionTypeConfig, actionType, formType]);

  const normalizeFieldOptions = (apiField: any) => {
    if (!apiField?.options) return undefined;
    return apiField.options?.map((option: any) => {
      const value = option?.valueID?.selected || option?.value || option?.label || '';
      return {
        label: option.label || '',
        valueID: { selected: value },
        fieldsUnderOption: option.fieldsUnderOption,
      };
    });
  };

  const actionConfig = useMemo(() => {
    if (!apiConfig) return null;
    return {
      actionType: apiConfig.actionType,
      formType: apiConfig.formType,
      title: apiConfig.title,
      version: apiConfig.version,
      fields: apiConfig.fields.map((apiField: any) => ({
        component: apiField.component,
        name: apiField.name,
        label: apiField.label,
        value: typeof apiField.value === 'string' ? apiField.value : '',
        checkboxValue: apiField.checkboxValue,
        valueID: apiField.valueID,
        options: normalizeFieldOptions(apiField),
        required: apiField.required,
        maxLength: apiField.maxLength,
        timeOptions: apiField.timeOptions,
        conditionalDisplay: apiField.conditionalDisplay,
        listItemFields: apiField.listItemFields?.map((itemField: any) => ({
          component: itemField.component,
          name: itemField.name,
          label: itemField.label,
          value: typeof itemField.value === 'string' ? itemField.value : '',
          validation: itemField.validation,
        })),
        ordinals: apiField.ordinals,
        listItemLabels: apiField.listItemLabels,
        validation: apiField.validation,
        sideEffects: apiField.sideEffects,
        receiptLabel: apiField.receiptLabel,
      })),
    };
  }, [apiConfig]);

  const actionData = useMemo(() => {
    if (!action?.data) return null;
    const rawData = action.data;
    const rawFields = Array.isArray(rawData.fields) ? rawData.fields : [];
    const fields = rawFields.reduce((acc: any[], f: any) => {
      if (f.component === 'datepicker' && f.value) {
        const match = String(f.value).match(/^(\d{2})\/(\d{2})-(\d{4})$/);
        if (match) {
          const [, dd, mm, yyyy] = match;
          const date = new Date(Number(yyyy), Number(mm) - 1, Number(dd), 12, 0, 0);
          const iso = Number.isNaN(date.getTime()) ? String(f.value) : date.toISOString();
          acc.push({ component: f.component, name: f.name, label: f.label, value: iso });
        } else {
          acc.push({ component: f.component, name: f.name, label: f.label, value: String(f.value) });
        }
      } else if (f.component === 'list') {
        const normalizedName = f.name.replace('Item', '');
        const existingIndex = acc.findIndex((a: any) => a.name === normalizedName);
        const valueIdItems = f.valueID?.items;
        const newItem = {
          ordinal: f.label,
          fields: Array.isArray(valueIdItems)
            ? valueIdItems.reduce((itemAcc: any, item: any) => { itemAcc[item.name] = item.value; return itemAcc; }, {})
            : {},
        };
        if (existingIndex !== -1) {
          const existing = acc[existingIndex];
          acc[existingIndex] = { ...existing, value: JSON.stringify([...JSON.parse(existing.value), newItem]) };
        } else {
          acc.push({ component: f.component, name: normalizedName, label: f.label, value: JSON.stringify([newItem]) });
        }
      } else if (f.component === 'files') {
        const valueIdFiles = f.valueID?.files;
        const valueFromValueId = Array.isArray(valueIdFiles)
          ? valueIdFiles.map((file: any) => file.name).filter(Boolean).join(', ')
          : '';
        const value = typeof f.value === 'string' && f.value ? f.value : valueFromValueId;
        acc.push({ component: f.component, name: f.name, label: f.label, value: value || '', valueID: f.valueID ?? undefined });
      } else {
        acc.push({ component: f.component || '', name: f.name || '', label: f.label || '', value: String(f.value || '') });
      }
      return acc;
    }, []);
    return { section: rawData.section || actionType, headers: rawData.headers || { version: '1.0' }, fields };
  }, [action, actionType]);

  return { actionConfig, actionData };
};
