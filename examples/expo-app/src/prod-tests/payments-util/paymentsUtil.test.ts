import { test, group, expect } from 'hermes-test';
// Production test port: paymentsUtil — 45 tests (pure functions)
// Original: apps/topdanmark/src/areas/Payments/helpers/__tests__/paymentsUtil.test.ts
//
// Zero mocks. Real business logic running in Hermes.
// Danish locale, moment-timezone, invoice processing.

import moment from 'moment-timezone';

import {
  getPaymentKeyFromType,
  getInvoicePaymentMethodLabel,
  hasReminderText,
  validateIfDueDateHasPassed,
  isPaymentDateInFuture,
  sortInvoicesByPeriodName,
  sumAmount,
  determineMonthOrYear,
  compareStringArrays,
  getDateTimeDifference,
  findIdOfDateClosestToToday,
  mergeValuesPairsWithSameName,
  getPaymentDate,
  getFormattedPaymentDate,
  getMapEntryKey,
  INVOICE_PAYMENT_METHODS_LABEL,
} from './util';

// =============================================
group('getPaymentKeyFromType', () => {
  test('returns paymentTypesGiro for GIRO', () => {
    expect(getPaymentKeyFromType(INVOICE_PAYMENT_METHODS_LABEL.GIRO)).toBe('paymentTypesGiro');
  });
  test('returns paymentTypesPbs for PAYMENT_SERVICE', () => {
    expect(getPaymentKeyFromType(INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE)).toBe('paymentTypesPbs');
  });
  test('returns paymentTypesMps for MOBILE_PAY', () => {
    expect(getPaymentKeyFromType(INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY)).toBe('paymentTypesMps');
  });
  test('returns empty string for UNKNOWN', () => {
    expect(getPaymentKeyFromType(INVOICE_PAYMENT_METHODS_LABEL.UNKNOWN)).toBe('');
  });
});

// =============================================
group('getInvoicePaymentMethodLabel', () => {
  test('returns PAYMENT_SERVICE', () => {
    expect(getInvoicePaymentMethodLabel('Betalingsservice')).toBe(INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE);
    expect(getInvoicePaymentMethodLabel('PBS')).toBe(INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE);
    expect(getInvoicePaymentMethodLabel('Tilmeldt Betalingsservice')).toBe(INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE);
    expect(getInvoicePaymentMethodLabel('BS_Top')).toBe(INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE);
  });
  test('returns GIRO', () => {
    expect(getInvoicePaymentMethodLabel('Giro')).toBe(INVOICE_PAYMENT_METHODS_LABEL.GIRO);
    expect(getInvoicePaymentMethodLabel('GIRO')).toBe(INVOICE_PAYMENT_METHODS_LABEL.GIRO);
    expect(getInvoicePaymentMethodLabel('FIK')).toBe(INVOICE_PAYMENT_METHODS_LABEL.GIRO);
    expect(getInvoicePaymentMethodLabel('FIK_Top')).toBe(INVOICE_PAYMENT_METHODS_LABEL.GIRO);
    expect(getInvoicePaymentMethodLabel('Betaling opkræves via girokort')).toBe(INVOICE_PAYMENT_METHODS_LABEL.GIRO);
    expect(getInvoicePaymentMethodLabel('EAN_Top')).toBe(INVOICE_PAYMENT_METHODS_LABEL.GIRO);
  });
  test('returns MOBILE_PAY', () => {
    expect(getInvoicePaymentMethodLabel('MobilePay')).toBe(INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY);
    expect(getInvoicePaymentMethodLabel('MobilePay Subscription')).toBe(INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY);
    expect(getInvoicePaymentMethodLabel('MobilePay-aftale')).toBe(INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY);
    expect(getInvoicePaymentMethodLabel('Mobilepay_Top')).toBe(INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY);
  });
});

// =============================================
group('hasReminderText', () => {
  test('returns true for strings containing "Rykkergebyr for opgørelsen"', () => {
    expect(hasReminderText('This contains Rykkergebyr for opgørelsen')).toBe(true);
    expect(hasReminderText('RYKKERGEBYR FOR OPGØRELSEN is present')).toBe(true);
    expect(hasReminderText('rykkergebyr for opgørelsen at the start')).toBe(true);
  });
  test('returns false for strings not containing the phrase', () => {
    expect(hasReminderText('This does not contain the phrase')).toBe(false);
    expect(hasReminderText('')).toBe(false);
    expect(hasReminderText('Rykkergebyr')).toBe(false);
  });
});

// =============================================
group('validateIfDueDateHasPassed', () => {
  test('returns true if due date has passed', () => {
    const pastDate = new Date();
    pastDate.setDate(pastDate.getDate() - 1);
    expect(validateIfDueDateHasPassed(pastDate.toISOString())).toBe(true);
  });
  test('returns false if due date is today', () => {
    expect(validateIfDueDateHasPassed(new Date().toISOString())).toBe(false);
  });
  test('returns false if due date is in the future', () => {
    const futureDate = new Date();
    futureDate.setDate(futureDate.getDate() + 1);
    expect(validateIfDueDateHasPassed(futureDate.toISOString())).toBe(false);
  });
});

// =============================================
group('isPaymentDateInFuture', () => {
  test('returns true for future date', () => {
    const futureDate = moment().add(1, 'days').format('DD/MM/YYYY');
    expect(isPaymentDateInFuture({ paymentDate: futureDate } as any)).toBe(true);
  });
  test('returns false for past date', () => {
    const pastDate = moment().subtract(1, 'days').format('DD/MM/YYYY');
    expect(isPaymentDateInFuture({ paymentDate: pastDate } as any)).toBe(false);
  });
  test('returns false for today', () => {
    const today = moment().format('DD/MM/YYYY');
    expect(isPaymentDateInFuture({ paymentDate: today } as any)).toBe(false);
  });
});

// =============================================
group('sortInvoicesByPeriodName', () => {
  test('sorts by year and month', () => {
    const invoices = [
      { periodTitle: 'Maj 2022' }, { periodTitle: 'April 2023' },
      { periodTitle: 'Marts 2022' }, { periodTitle: 'Januar 2023' },
    ] as any[];
    const sorted = sortInvoicesByPeriodName(invoices);
    expect(sorted.map((i: any) => i.periodTitle)).toEqual(['Marts 2022', 'Maj 2022', 'Januar 2023', 'April 2023']);
  });
  test('handles same month and year', () => {
    const invoices = [{ periodTitle: 'Januar 2023' }, { periodTitle: 'Januar 2023' }] as any[];
    const sorted = sortInvoicesByPeriodName(invoices);
    expect(sorted.map((i: any) => i.periodTitle)).toEqual(['Januar 2023', 'Januar 2023']);
  });
  test('sorts within same year', () => {
    const invoices = [
      { periodTitle: 'December 2022' }, { periodTitle: 'Januar 2022' }, { periodTitle: 'Marts 2022' },
    ] as any[];
    const sorted = sortInvoicesByPeriodName(invoices);
    expect(sorted.map((i: any) => i.periodTitle)).toEqual(['Januar 2022', 'Marts 2022', 'December 2022']);
  });
  test('sorts across years with same month', () => {
    const invoices = [
      { periodTitle: 'December 2022' }, { periodTitle: 'Januar 2023' },
      { periodTitle: 'Januar 2022' }, { periodTitle: 'Marts 2022' },
    ] as any[];
    const sorted = sortInvoicesByPeriodName(invoices);
    expect(sorted.map((i: any) => i.periodTitle)).toEqual(['Januar 2022', 'Marts 2022', 'December 2022', 'Januar 2023']);
  });
});

// =============================================
group('sumAmount', () => {
  test('sums total payment amounts', () => {
    const entries = [{ totalPaymentAmount: 100.01 }, { totalPaymentAmount: 200.02 }, { totalPaymentAmount: 300.03 }] as any[];
    expect(sumAmount(entries)).toBe(600.06);
  });
  test('returns 0 for empty array', () => {
    expect(sumAmount([])).toBe(0);
  });
  test('handles negative amounts', () => {
    const entries = [{ totalPaymentAmount: -100.01 }, { totalPaymentAmount: 200.02 }] as any[];
    expect(sumAmount(entries)).toBe(100.01);
  });
  test('rounds to two decimal places', () => {
    const entries = [{ totalPaymentAmount: 100.001 }, { totalPaymentAmount: 200.002 }, { totalPaymentAmount: 300.003 }] as any[];
    expect(sumAmount(entries)).toBe(600.01);
  });
});

// =============================================
group('determineMonthOrYear', () => {
  test('returns year when all invoices from different years', () => {
    expect(determineMonthOrYear({
      guidewireInvoices: [
        { paymentDetails: { isInvoicePaid: true, paymentDate: { year: 2020 }, paidDate: null }, invoiceOverview: { dueDate: { year: 2020 } } },
        { paymentDetails: { isInvoicePaid: false }, invoiceOverview: { dueDate: { year: 2021 } } },
      ],
      primoInvoices: [
        { invoiceStatus: 'Betalt', invoicePaymentDate: '2022-01-01', invoiceDueDate: '2022-01-01' },
        { invoiceStatus: 'Ikke Betalt', invoiceDueDate: '2023-01-01' },
      ],
    })).toBe('year');
  });
  test('returns month when a year repeats', () => {
    expect(determineMonthOrYear({
      guidewireInvoices: [
        { paymentDetails: { isInvoicePaid: true, paymentDate: { year: 2020 }, paidDate: null }, invoiceOverview: { dueDate: { year: 2020 } } },
        { paymentDetails: { isInvoicePaid: false }, invoiceOverview: { dueDate: { year: 2020 } } },
      ],
      primoInvoices: [
        { invoiceStatus: 'Betalt', invoicePaymentDate: '2021-01-01', invoiceDueDate: '2021-01-01' },
        { invoiceStatus: 'Ikke Betalt', invoiceDueDate: '2022-01-01' },
      ],
    })).toBe('month');
  });
  test('returns year for empty arrays', () => {
    expect(determineMonthOrYear({ guidewireInvoices: [], primoInvoices: [], topproInvoices: [] })).toBe('year');
  });
  test('returns month when same year in paid and unpaid', () => {
    expect(determineMonthOrYear({
      guidewireInvoices: [
        { paymentDetails: { isInvoicePaid: true, paymentDate: { year: 2021 }, paidDate: null }, invoiceOverview: { dueDate: { year: 2021 } } },
      ],
      primoInvoices: [{ invoiceStatus: 'Ikke Betalt', invoiceDueDate: '2021-01-01' }],
    })).toBe('month');
  });
  test('returns month if same year occurs multiple times', () => {
    expect(determineMonthOrYear({
      guidewireInvoices: [
        { paymentDetails: { isInvoicePaid: true, paymentDate: null, paidDate: { year: 2021 } }, invoiceOverview: { dueDate: { year: 2021 } } },
        { paymentDetails: { isInvoicePaid: true, paymentDate: null, paidDate: { year: 2021 } }, invoiceOverview: { dueDate: { year: 2021 } } },
      ],
      primoInvoices: [
        { invoiceStatus: 'Betalt', invoicePaymentDate: '2021-01-01' },
        { invoiceStatus: 'Betalt', invoicePaymentDate: '2021-01-01' },
        { invoiceStatus: 'Betalt', invoicePaymentDate: null },
      ],
    })).toBe('month');
  });
  test('returns year when paymentDate and paidDate are null', () => {
    expect(determineMonthOrYear({
      guidewireInvoices: [
        { paymentDetails: { isInvoicePaid: true, paymentDate: null, paidDate: null }, invoiceOverview: { dueDate: { year: 2021 } } },
      ],
      primoInvoices: [],
    })).toBe('year');
  });
});

// =============================================
group('compareStringArrays', () => {
  test('returns true for identical arrays', () => {
    expect(compareStringArrays(['apple', 'banana', 'cherry'], ['apple', 'banana', 'cherry'])).toBe(true);
  });
  test('returns false for different lengths', () => {
    expect(compareStringArrays(['apple', 'banana'], ['apple', 'banana', 'cherry'])).toBe(false);
  });
  test('returns false for different elements', () => {
    expect(compareStringArrays(['apple', 'banana', 'cherry'], ['apple', 'grape', 'cherry'])).toBe(false);
  });
  test('returns true for two empty arrays', () => {
    expect(compareStringArrays([], [])).toBe(true);
  });
});

// =============================================
group('getDateTimeDifference', () => {
  test('returns 0 for same-day dates', () => {
    const d1 = new Date('2023-01-01T00:00:00Z');
    const d2 = new Date('2023-01-01T01:00:00Z');
    expect(getDateTimeDifference(d1, d2)).toBe(0);
  });
  test('returns 0 for identical dates', () => {
    const d = new Date('2023-01-01T00:00:00Z');
    expect(getDateTimeDifference(d, d)).toBe(0);
  });
  test('handles first date after second', () => {
    const d1 = new Date('2023-01-01T02:00:00Z');
    const d2 = new Date('2023-01-01T01:00:00Z');
    expect(getDateTimeDifference(d1, d2)).toBe(0);
  });
});

// =============================================
group('findIdOfDateClosestToToday', () => {
  test('returns id of closest date', () => {
    const today = moment().format('DD/MM/YYYY');
    const yesterday = moment().subtract(1, 'days').format('DD/MM/YYYY');
    const tomorrow = moment().add(1, 'days').format('DD/MM/YYYY');
    expect(findIdOfDateClosestToToday([
      ['1', [{ paymentDate: yesterday }]], ['2', [{ paymentDate: today }]], ['3', [{ paymentDate: tomorrow }]],
    ] as any)).toBe('2');
  });
  test('returns empty string for empty array', () => {
    expect(findIdOfDateClosestToToday([])).toBe('');
  });
  test('returns first id when multiple equally close', () => {
    const today = moment().format('DD/MM/YYYY');
    const yesterday = moment().subtract(1, 'days').format('DD/MM/YYYY');
    expect(findIdOfDateClosestToToday([
      ['1', [{ paymentDate: yesterday }]], ['2', [{ paymentDate: today }]], ['3', [{ paymentDate: yesterday }]],
    ] as any)).toBe('2');
  });
  test('returns closest future date', () => {
    const tomorrow = moment().add(1, 'days').format('DD/MM/YYYY');
    const dayAfter = moment().add(2, 'days').format('DD/MM/YYYY');
    expect(findIdOfDateClosestToToday([
      ['1', [{ paymentDate: dayAfter }]], ['2', [{ paymentDate: tomorrow }]],
    ] as any)).toBe('2');
  });
  test('handles undefined payment date', () => {
    expect(findIdOfDateClosestToToday([['1', [{ paymentDate: undefined }]]] as any)).toBe('1');
  });
  test('returns closest past date', () => {
    const yesterday = moment().subtract(1, 'days').format('DD/MM/YYYY');
    const dayBefore = moment().subtract(2, 'days').format('DD/MM/YYYY');
    expect(findIdOfDateClosestToToday([
      ['1', [{ paymentDate: dayBefore }]], ['2', [{ paymentDate: yesterday }]],
    ] as any)).toBe('2');
  });
});

// =============================================
group('mergeValuesPairsWithSameName', () => {
  test('merges pairs with same name by summing values', () => {
    expect(mergeValuesPairsWithSameName([
      [{ label: 'A', value: '10' }, { label: 'B', value: '20' }],
      [{ label: 'A', value: '30' }, { label: 'C', value: '40' }],
    ])).toEqual([
      { label: 'A', value: '40' }, { label: 'B', value: '20' }, { label: 'C', value: '40' },
    ]);
  });
  test('handles empty input', () => {
    expect(mergeValuesPairsWithSameName([])).toEqual([]);
  });
  test('does not modify original array', () => {
    const pairs = [[{ label: 'A', value: '10' }]];
    const copy = JSON.parse(JSON.stringify(pairs));
    mergeValuesPairsWithSameName(pairs);
    expect(pairs).toEqual(copy);
  });
  test('treats non-numeric values as zeros', () => {
    expect(mergeValuesPairsWithSameName([
      [{ label: 'A', value: 'abc' }, { label: 'B', value: '20' }],
      [{ label: 'A', value: '30' }, { label: 'C', value: 'xyz' }],
    ])).toEqual([
      { label: 'A', value: '30' }, { label: 'B', value: '20' }, { label: 'C', value: '0' },
    ]);
    expect(mergeValuesPairsWithSameName([
      [{ label: 'A', value: 'abc' }, { label: 'B', value: 'asdv' }],
      [{ label: 'A', value: 'sdsda' }, { label: 'C', value: 'ssss' }],
    ])).toEqual([
      { label: 'A', value: '0' }, { label: 'B', value: '0' }, { label: 'C', value: '0' },
    ]);
  });
});

// =============================================
group('payment date formatting', () => {
  test('getPaymentDate formats correctly', () => {
    // Hermes da-DK locale: "15. feb. 2024" (abbreviated)
    const result = getPaymentDate(new Date(2024, 1, 15));
    expect(result).toContain('15');
    expect(result).toContain('2024');
  });
  test('getFormattedPaymentDate formats with month name', () => {
    const result = getFormattedPaymentDate(new Date(2024, 1, 15));
    expect(result).toContain('15');
    expect(result).toContain('2024');
    expect(result).toContain('feb'); // feb or februar depending on engine
  });
});

// =============================================
group('getMapEntryKey', () => {
  test('returns month + year for monthly', () => {
    expect(getMapEntryKey(new Date(2024, 1, 15), 'month')).toBe('Februar 2024');
  });
  test('returns year for yearly', () => {
    expect(getMapEntryKey(new Date(2024, 1, 15), 'year')).toBe('2024');
  });
});
