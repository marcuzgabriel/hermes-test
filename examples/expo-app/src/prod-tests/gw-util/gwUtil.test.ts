import { test, group, expect } from 'hermes-test';
// Production test port: gwUtil — 37 tests
// Original: apps/topdanmark/src/areas/Payments/helpers/__tests__/gwUtil.test.ts
//
// Pure functions. Guidewire invoice processing — dates, fees, invoice types, period mapping.

import moment from 'moment-timezone';
import {
  validateIfDueDateHasPassedGWDate, extractPeriodFromGWInvoice, getGWInvoiceTypes,
  getInsuranceFeesGuidewire, getMissingPaymentEntriesGWRecursively,
  getMomentFromGWDate, getPaymentPeriodNameOfNextInvoiceGW,
  resolveIfInvoiceConvertedToYearlyPaymentGW, populateMapWithGWInvoicePeriods,
  InvoiceType,
} from './gwUtil';

// --- Mock invoice data (inline, matches production __mocks__) ---
const basicGWInvoice = {
  invoiceOverview: {
    invoiceNumber: 'INV-001',
    billDate: { year: 2024, month: 6, day: 1 },
    dueDate: { year: 2024, month: 7, day: 15 },
    totalDueAmount: { amount: 1500 },
    insurances: [{ insuranceName: 'Bilforsikring', startDate: { day: 1, month: 1, year: 2024 } }],
    invoiceType: 'Normal',
    isLawyerLetterSent: false,
    invoiceReplacedBy: null,
    fromPriorInvoice: null,
    taxesAndFees: [],
  },
  paymentDetails: {
    isInvoicePaid: true, paymentPlan: 'monthly',
    paymentDate: { year: 2024, month: 7, day: 10 },
    paidDate: null,
  },
  invoiceDetails: [{ insuranceAmount: { amount: 1500 } }],
};

group('validateIfDueDateHasPassedGWDate', () => {
  test('returns true for past date', () => {
    expect(validateIfDueDateHasPassedGWDate({ year: 2023, month: 9, day: 15 })).toBe(true);
  });
  test('returns false for future date', () => {
    expect(validateIfDueDateHasPassedGWDate({ year: 2069, month: 9, day: 15 })).toBe(false);
  });
});

group('extractPeriodFromGWInvoice', () => {
  test('returns empty for null', () => {
    expect(extractPeriodFromGWInvoice(null)).toBe('');
  });
  test('returns formatted period', () => {
    const invoice = { startDate: { day: 1, month: 1, year: 2020 }, endDate: { day: 31, month: 12, year: 2020 } };
    const result = extractPeriodFromGWInvoice(invoice);
    expect(result).toContain('2020');
    expect(result).toContain('-');
  });
  test('returns empty if missing start or end', () => {
    expect(extractPeriodFromGWInvoice({ startDate: { day: 1, month: 1, year: 2020 } })).toBe('');
    expect(extractPeriodFromGWInvoice({ endDate: { day: 31, month: 12, year: 2020 } })).toBe('');
  });
});

group('getGWInvoiceTypes', () => {
  test('returns NORMAL for basic invoice', () => {
    expect(getGWInvoiceTypes(basicGWInvoice)).toEqual([InvoiceType.NORMAL]);
  });
  test('returns OVERDUE for unpaid past-due', () => {
    const inv = { ...basicGWInvoice, paymentDetails: { ...basicGWInvoice.paymentDetails, isInvoicePaid: false },
      invoiceOverview: { ...basicGWInvoice.invoiceOverview, dueDate: { year: 2020, month: 1, day: 1 } } };
    expect(getGWInvoiceTypes(inv)).toContain(InvoiceType.OVERDUE);
  });
  test('returns EXPIRED for lawyer letter', () => {
    const inv = { ...basicGWInvoice, invoiceOverview: { ...basicGWInvoice.invoiceOverview, isLawyerLetterSent: true } };
    expect(getGWInvoiceTypes(inv)).toContain(InvoiceType.EXPIRED);
  });
  test('returns REMINDER for rykkergebyr', () => {
    const inv = { ...basicGWInvoice, invoiceOverview: { ...basicGWInvoice.invoiceOverview,
      taxesAndFees: [{ taxFeeAmount: { amount: 100 }, taxFeeText: 'Rykkergebyr', category: 'fee' }] } };
    expect(getGWInvoiceTypes(inv)).toContain(InvoiceType.REMINDER);
  });
  test('returns REPLACED', () => {
    const inv = { ...basicGWInvoice, invoiceOverview: { ...basicGWInvoice.invoiceOverview, invoiceReplacedBy: 'INV-002' } };
    expect(getGWInvoiceTypes(inv)).toContain(InvoiceType.REPLACED);
  });
});

group('getInsuranceFeesGuidewire', () => {
  test('filters and calculates fees by insurance name', () => {
    const fees = [
      { feeName: 'ProcessingFee', totalAmount: 200, flatFeeDetails: [{ insuranceName: 'A' }, { insuranceName: 'B' }] },
      { feeName: 'ServiceCharge', totalAmount: 100, flatFeeDetails: [{ insuranceName: 'A' }] },
    ];
    const result = getInsuranceFeesGuidewire(fees, { insuranceName: 'A' });
    expect(result.feeEntries).toEqual([{ label: 'ProcessingFee', value: '100' }, { label: 'ServiceCharge', value: '100' }]);
    expect(result.totalFeeValue).toBe(200);
  });
  test('handles no matching insurance', () => {
    const fees = [{ feeName: 'Fee', totalAmount: 50, flatFeeDetails: [{ insuranceName: 'C' }] }];
    const result = getInsuranceFeesGuidewire(fees, { insuranceName: 'A' });
    expect(result.feeEntries).toEqual([]);
    expect(result.totalFeeValue).toBe(0);
  });
  test('handles empty fees', () => {
    const result = getInsuranceFeesGuidewire([], { insuranceName: 'A' });
    expect(result.feeEntries).toEqual([]);
    expect(result.totalFeeValue).toBe(0);
  });
  test('handles multiple entries for same insurance', () => {
    const fees = [{ feeName: 'DocFee', totalAmount: 300, flatFeeDetails: [{ insuranceName: 'A' }, { insuranceName: 'A' }, { insuranceName: 'B' }] }];
    const result = getInsuranceFeesGuidewire(fees, { insuranceName: 'A' });
    expect(result.feeEntries).toEqual([{ label: 'DocFee', value: '100' }]);
    expect(result.totalFeeValue).toBe(100);
  });
});

group('getMissingPaymentEntriesGWRecursively', () => {
  const t = (key: string) => key;

  test('returns entries if no fromPriorInvoice', () => {
    const inv = { invoiceOverview: { fromPriorInvoice: null } };
    expect(getMissingPaymentEntriesGWRecursively([], inv, [], t)).toEqual([]);
  });
  test('returns entries if prior not found', () => {
    const inv = { invoiceOverview: { fromPriorInvoice: { priorInvoiceNumber: '123' } } };
    expect(getMissingPaymentEntriesGWRecursively([], inv, [], t)).toEqual([]);
  });
  test('adds monthly entry with lowercase month', () => {
    const current = { invoiceOverview: { fromPriorInvoice: { priorInvoiceNumber: '123' } } };
    const prior = { invoiceOverview: { invoiceNumber: '123', dueDate: { month: 5 }, totalDueAmount: { amount: 100 } }, paymentDetails: { paymentPlan: 'monthly' } };
    const result = getMissingPaymentEntriesGWRecursively([prior], current, [], t);
    expect(result[0].invoiceName).toBe('paymentFor maj');
    expect(result[0].invoiceTotalAmount).toBe(100);
  });
  test('adds yearly entry with year', () => {
    const current = { invoiceOverview: { fromPriorInvoice: { priorInvoiceNumber: '123' } } };
    const prior = { invoiceOverview: { invoiceNumber: '123', dueDate: { year: 2022 }, totalDueAmount: { amount: 100 } }, paymentDetails: { paymentPlan: 'annual' } };
    const result = getMissingPaymentEntriesGWRecursively([prior], current, [], t);
    expect(result[0].invoiceName).toBe('paymentFor 2022');
  });
});

group('getMomentFromGWDate', () => {
  test('converts GWDate to moment', () => {
    const dates = [
      { gwDate: { year: 2022, month: 1, day: 1 }, expected: '1-1-2022' },
      { gwDate: { year: 2024, month: 12, day: 1 }, expected: '1-12-2024' },
    ];
    for (const { gwDate, expected } of dates) {
      const result = getMomentFromGWDate(gwDate);
      const exp = moment(expected, 'DD-MM-YYYY');
      expect(result.format('YYYY-MM-DD')).toBe(exp.format('YYYY-MM-DD'));
    }
  });
});

group('getPaymentPeriodNameOfNextInvoiceGW', () => {
  test('returns null if no invoiceReplacedBy', () => {
    expect(getPaymentPeriodNameOfNextInvoiceGW([], { invoiceOverview: { invoiceReplacedBy: null } })).toBeNull();
  });
  test('returns null if newer not found', () => {
    expect(getPaymentPeriodNameOfNextInvoiceGW([], { invoiceOverview: { invoiceReplacedBy: '123' } })).toBeNull();
  });
  test('returns month name for valid dueDate', () => {
    const newer = { invoiceOverview: { invoiceNumber: '123', dueDate: { month: 5 }, insurances: [{}] } };
    expect(getPaymentPeriodNameOfNextInvoiceGW([newer], { invoiceOverview: { invoiceReplacedBy: '123' } })).toBe('maj');
  });
  test('returns null for invalid month', () => {
    const newer = { invoiceOverview: { invoiceNumber: '123', dueDate: { month: 14 }, insurances: [{}] } };
    expect(getPaymentPeriodNameOfNextInvoiceGW([newer], { invoiceOverview: { invoiceReplacedBy: '123' } })).toBeNull();
  });
});

group('resolveIfInvoiceConvertedToYearlyPaymentGW', () => {
  const yearly = { paymentDetails: { paymentPlan: 'yearly' }, invoiceOverview: { fromPriorInvoice: { priorInvoiceNumber: '123' } } };
  const monthly = { paymentDetails: { paymentPlan: 'monthly' }, invoiceOverview: { invoiceNumber: '123' } };

  test('returns false if not yearly', () => {
    const nonYearly = { paymentDetails: { paymentPlan: 'monthly' }, invoiceOverview: { fromPriorInvoice: { priorInvoiceNumber: '123' } } };
    expect(resolveIfInvoiceConvertedToYearlyPaymentGW([monthly], nonYearly)).toBe(false);
  });
  test('returns false if no prior invoice', () => {
    const noPrior = { paymentDetails: { paymentPlan: 'yearly' }, invoiceOverview: {} };
    expect(resolveIfInvoiceConvertedToYearlyPaymentGW([monthly], noPrior)).toBe(false);
  });
  test('returns false if prior not found', () => {
    expect(resolveIfInvoiceConvertedToYearlyPaymentGW([], yearly)).toBe(false);
  });
  test('returns true if prior was monthly', () => {
    expect(resolveIfInvoiceConvertedToYearlyPaymentGW([monthly], yearly)).toBe(true);
  });
  test('returns false if prior was yearly', () => {
    const yearlyPrior = { paymentDetails: { paymentPlan: 'yearly' }, invoiceOverview: { invoiceNumber: '123' } };
    expect(resolveIfInvoiceConvertedToYearlyPaymentGW([yearlyPrior], yearly)).toBe(false);
  });
});

group('populateMapWithGWInvoicePeriods', () => {
  test('populates map from invoices', () => {
    const map = new Map();
    populateMapWithGWInvoicePeriods({ guidewireInvoices: [basicGWInvoice] }, 'month', map, (k: string) => k, '1');
    expect(map.size).toBe(1);
  });
  test('handles empty invoices', () => {
    const map = new Map();
    populateMapWithGWInvoicePeriods({ guidewireInvoices: [] }, 'month', map, (k: string) => k, '1');
    expect(map.size).toBe(0);
  });
  test('handles two invoices', () => {
    const map = new Map();
    populateMapWithGWInvoicePeriods({ guidewireInvoices: [basicGWInvoice, basicGWInvoice] }, 'month', map, (k: string) => k, '1');
    expect(map.size).toBe(1);
  });
});
