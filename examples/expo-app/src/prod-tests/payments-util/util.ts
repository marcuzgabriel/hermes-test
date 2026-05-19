// Real production code — faithful copy from insurance app
// Only change: inline danishMonths + enums (no monorepo imports)
import moment from 'moment-timezone';

// Capitalized — matches production useGetInsuranceFormatHelpers.ts
export const danishMonths = [
  'Januar', 'Februar', 'Marts', 'April', 'Maj', 'Juni',
  'Juli', 'August', 'September', 'Oktober', 'November', 'December',
];

export enum INVOICE_PAYMENT_METHODS_LABEL {
  GIRO = 'GIRO',
  PAYMENT_SERVICE = 'PAYMENT_SERVICE',
  MOBILE_PAY = 'MOBILE_PAY',
  MOBILE_PAY_SUBSCRIPTION = 'MOBILE_PAY_SUBSCRIPTION',
  UNKNOWN = 'UNKNOWN',
}

export enum InvoiceRefundState { NONE = 'NONE', SINGLE = 'SINGLE', MULTIPLE = 'MULTIPLE' }
export enum InvoiceType { REMINDER = 'REMINDER', OVERDUE = 'OVERDUE', NORMAL = 'NORMAL' }

export interface InvoicePeriodEntry {
  totalPaymentAmount: number;
  paymentDate?: string;
  invoiceTypeAttributes: string[];
  periodInvoices: Array<{ invoiceIsRefund: boolean }>;
  replacedByInvoiceNumber?: string;
  refundState?: string;
  [key: string]: any;
}

export interface PaymentsOverview {
  periodTitle: string;
  periodType: string;
  periodLabel: string;
  periodEntries: InvoicePeriodEntry[];
  isFutureOrPresent: string;
  totalPeriodAmount: number;
  isDefaultSelected: boolean;
  hasSingleEntryRefund: boolean;
}

export interface ValuePair { label: string; value: string; }

export const getInvoicePaymentMethodLabel = (paymentTypeExtracted: string): INVOICE_PAYMENT_METHODS_LABEL => {
  switch (paymentTypeExtracted) {
    case 'Betalingsservice': case 'PBS': case 'Tilmeldt Betalingsservice': case 'BS_Top':
      return INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE;
    case 'Giro': case 'GIRO': case 'FIK': case 'FIK_Top': case 'Betaling opkræves via girokort': case 'EAN_Top':
      return INVOICE_PAYMENT_METHODS_LABEL.GIRO;
    case 'MobilePay': case 'MobilePay Subscription': case 'MobilePay-aftale': case 'Mobilepay_Top': case 'MP':
      return INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY;
  }
  return INVOICE_PAYMENT_METHODS_LABEL.UNKNOWN;
};

export const getPaymentKeyFromType = (paymentType: INVOICE_PAYMENT_METHODS_LABEL) => {
  switch (paymentType) {
    case INVOICE_PAYMENT_METHODS_LABEL.GIRO: return 'paymentTypesGiro';
    case INVOICE_PAYMENT_METHODS_LABEL.PAYMENT_SERVICE: return 'paymentTypesPbs';
    case INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY:
    case INVOICE_PAYMENT_METHODS_LABEL.MOBILE_PAY_SUBSCRIPTION: return 'paymentTypesMps';
    case INVOICE_PAYMENT_METHODS_LABEL.UNKNOWN: return '';
  }
};

export const hasReminderText = (inputString: string) =>
  /Rykkergebyr for opgørelsen/i.test(inputString);

export const validateIfDueDateHasPassed = (paymentDueDate: string) =>
  new Date().setHours(0, 0, 0, 0) > new Date(paymentDueDate).setHours(0, 0, 0, 0);

export const isPaymentDateInFuture = (entry: InvoicePeriodEntry) =>
  moment(entry.paymentDate, 'DD/MM/YYYY').toDate().getTime() > new Date().getTime();

export const sortInvoicesByPeriodName = (invoices: PaymentsOverview[]) => {
  return invoices.sort((a, b) => {
    const aYear = a.periodTitle.substring(a.periodTitle.length - 4);
    const bYear = b.periodTitle.substring(b.periodTitle.length - 4);
    if (aYear !== bYear) return Number(aYear) - Number(bYear);
    const aMonth = danishMonths.indexOf(a.periodTitle.substring(0, a.periodTitle.length - 5));
    const bMonth = danishMonths.indexOf(b.periodTitle.substring(0, b.periodTitle.length - 5));
    return aMonth - bMonth;
  });
};

export const sumAmount = (entries: InvoicePeriodEntry[]) =>
  Number(entries.reduce((acc, p) => acc + p.totalPaymentAmount, 0).toFixed(2));

export const determineMonthOrYear = (invoiceOverview: any): 'year' | 'month' => {
  let periodType: 'year' | 'month' = 'year';
  const yearsDiscovered: number[] = [];

  for (const gw of (invoiceOverview.guidewireInvoices ?? [])) {
    if (gw.paymentDetails.isInvoicePaid) {
      const y = gw.paymentDetails.paymentDate?.year ?? gw.paymentDetails.paidDate?.year ?? null;
      if (y !== null) {
        if (!yearsDiscovered.includes(y)) yearsDiscovered.push(y);
        else { periodType = 'month'; break; }
      }
    } else {
      const y = gw.invoiceOverview.dueDate.year;
      if (!yearsDiscovered.includes(y)) yearsDiscovered.push(y);
      else { periodType = 'month'; break; }
    }
  }

  for (const primo of (invoiceOverview.primoInvoices ?? [])) {
    if (primo.invoiceStatus === 'Betalt') {
      const y = parseInt(primo.invoicePaymentDate?.substring(0, 4) ?? '', 10);
      if (!yearsDiscovered.includes(y)) yearsDiscovered.push(y);
      else { periodType = 'month'; break; }
    } else {
      const y = parseInt(primo.invoiceDueDate.substring(0, 4), 10);
      if (!yearsDiscovered.includes(y)) yearsDiscovered.push(y);
      else { periodType = 'month'; break; }
    }
  }

  return periodType;
};

export const compareStringArrays = (a: string[], b: string[]): boolean =>
  a.length === b.length && a.every(v => b.includes(v));

export const getDateTimeDifference = (date1: Date, date2: Date): number => {
  if (date1.getDate() === date2.getDate() && date1.getMonth() === date2.getMonth() && date1.getFullYear() === date2.getFullYear()) return 0;
  return Math.abs(date1.getTime() - date2.getTime());
};

export const findIdOfDateClosestToToday = (entries: [string, InvoicePeriodEntry[]][]): string => {
  let closestId = '';
  let closestDate: Date | null = null;
  const now = new Date();

  entries.forEach(([key, value]) => {
    const paymentDate = moment(value[0]?.paymentDate, 'DD/MM/YYYY').set('hour', 12).toDate();
    if (closestId === '') {
      closestId = key;
      closestDate = paymentDate;
    } else if (closestDate && getDateTimeDifference(paymentDate, now) < getDateTimeDifference(closestDate, now)) {
      closestDate = paymentDate;
      closestId = key;
    }
  });
  return closestId;
};

export const mergeValuesPairsWithSameName = (pairs: ValuePair[][]): ValuePair[] => {
  const merged: ValuePair[] = [];
  pairs.forEach(pair => {
    pair.forEach(p => {
      const existing = merged.find(m => m.label === p.label);
      let newValue = parseFloat(p.value);
      if (isNaN(newValue)) newValue = 0;
      if (existing) {
        existing.value = `${parseFloat(existing.value) + newValue}`;
      } else {
        merged.push({ ...p, value: `${newValue}` });
      }
    });
  });
  return merged;
};

export const getPaymentDate = (date: Date) => date.toLocaleDateString('da-DK');

export const getFormattedPaymentDate = (date: Date) =>
  date.toLocaleDateString('da-DK', { year: 'numeric', month: 'long', day: 'numeric' });

export const getMapEntryKey = (date: Date, periodType: 'year' | 'month') =>
  periodType === 'year'
    ? `${date.getFullYear()}`
    : `${danishMonths[date.getMonth()]} ${date.getFullYear()}`;
