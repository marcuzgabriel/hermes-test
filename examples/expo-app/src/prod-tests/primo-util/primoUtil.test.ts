import { test, group, expect } from 'hermes-test';
// Production test port: primoUtil — 15 tests
import { extractPeriodFromPrimoInvoice, getAllInsuranceTopLevelFeesPrimo, getPrimoInvoiceTypes, getAllMissingInvoicePaymentEntriesPrimo, getInsuranceAttachedFeesPrimo, populateMapWithPrimoInvoicePeriods, InvoiceType } from './primoUtil';

group('extractPeriodFromPrimoInvoice', () => {
  test('returns empty for null', () => { expect(extractPeriodFromPrimoInvoice(null)).toBe(''); });
  test('extracts and formats period', () => {
    const d = { paymentSpecifications: [{ specificationType: 'Normal', specificationLabel: 'Til gode inkl. afgifter for perioden 31. maj 2024 - 1. juli 2024' }] };
    expect(extractPeriodFromPrimoInvoice(d)).toBe('31. MAJ. 2024 - 1. JUL. 2024');
  });
  test('returns empty if no perioden', () => {
    expect(extractPeriodFromPrimoInvoice({ paymentSpecifications: [{ specificationType: 'Normal', specificationLabel: 'Random text' }] })).toBe('');
  });
  test('returns empty for empty specs', () => {
    expect(extractPeriodFromPrimoInvoice({ paymentSpecifications: [] })).toBe('');
  });
  test('returns empty if specificationLabel missing', () => {
    expect(extractPeriodFromPrimoInvoice({ paymentSpecifications: [{ specificationType: 'Normal' }] })).toBe('');
  });
});

group('getAllInsuranceTopLevelFeesPrimo', () => {
  test('merges fees from multiple details', () => {
    const inv = { invoiceDetails: [
      { paymentSpecifications: [{ specificationLabel: 'Opkrævningsgebyr', specificationValue: 100 }] },
      { paymentSpecifications: [{ specificationLabel: 'Opkrævningsgebyr', specificationValue: 100 }] },
    ]};
    expect(getAllInsuranceTopLevelFeesPrimo(inv)).toEqual([{ label: 'Opkrævningsgebyr', value: '200' }]);
  });
  test('handles empty details', () => {
    expect(getAllInsuranceTopLevelFeesPrimo({ invoiceDetails: [] })).toEqual([]);
  });
  test('handles no matching fees', () => {
    const inv = { invoiceDetails: [{ paymentSpecifications: [{ specificationLabel: 'Random', specificationValue: 50 }] }] };
    expect(getAllInsuranceTopLevelFeesPrimo(inv)).toEqual([]);
  });
});

group('getPrimoInvoiceTypes', () => {
  test('returns NORMAL for basic paid invoice', () => {
    const inv = { invoicePaidIndicator: true, invoiceDueDate: '2099-01-01', invoiceStatus: 'Betalt' };
    expect(getPrimoInvoiceTypes(inv)).toEqual([InvoiceType.NORMAL]);
  });
  test('returns CANCELLED', () => {
    const inv = { invoiceStatus: 'Annulleret', invoiceDueDate: '2024-01-01', invoicePaidIndicator: false };
    expect(getPrimoInvoiceTypes(inv)).toContain(InvoiceType.CANCELLED);
  });
  test('returns REMINDER', () => {
    const inv = { invoiceReminderDate: '2024-01-01', invoiceDueDate: '2099-01-01', invoicePaidIndicator: true, invoiceStatus: 'Betalt' };
    expect(getPrimoInvoiceTypes(inv)).toContain(InvoiceType.REMINDER);
  });
});

group('getInsuranceAttachedFeesPrimo', () => {
  test('extracts insurance-level fees', () => {
    const d = { paymentSpecifications: [{ specificationLabel: 'Forsikringsafgift', specificationValue: 50 }, { specificationLabel: 'Random', specificationValue: 10 }] };
    const result = getInsuranceAttachedFeesPrimo(d);
    expect(result.length).toBe(1);
    expect(result[0].label).toBe('Forsikringsafgift');
  });
});

group('populateMapWithPrimoInvoicePeriods', () => {
  test('populates map from primo invoices', () => {
    const map = new Map();
    populateMapWithPrimoInvoicePeriods({ primoInvoices: [{ invoiceDueDate: '2024-06-15', invoiceTotalAmount: 500, invoicePaidIndicator: true, invoiceStatus: 'Betalt' }] }, 'month', map, (k) => k);
    expect(map.size).toBe(1);
  });
  test('handles empty', () => {
    const map = new Map();
    populateMapWithPrimoInvoicePeriods({ primoInvoices: [] }, 'month', map, (k) => k);
    expect(map.size).toBe(0);
  });
});
