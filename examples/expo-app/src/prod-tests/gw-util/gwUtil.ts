// Real production code — faithful copy of gwUtil.ts
// Changes: inline deps (danishMonths, validateIfDueDateHasPassed, formatPrice, etc.)
import moment from 'moment-timezone';

const danishMonths = ['Januar','Februar','Marts','April','Maj','Juni','Juli','August','September','Oktober','November','December'];

export interface GWDate { year: number; month: number; day: number; }

export const getMomentFromGWDate = (gwDate: GWDate) =>
  moment(`${gwDate.day}-${gwDate.month}-${gwDate.year}`, 'DD-MM-YYYY');

export const validateIfDueDateHasPassed = (dateStr: string) =>
  new Date().setHours(0,0,0,0) > new Date(dateStr).setHours(0,0,0,0);

export const validateIfDueDateHasPassedGWDate = (paymentDueDate: GWDate) => {
  const converted = getMomentFromGWDate(paymentDueDate).toDate();
  return validateIfDueDateHasPassed(converted.toDateString());
};

export const hasYearlyTaxes = (taxesAndFees: any[] | undefined): boolean => {
  if (!taxesAndFees) return false;
  const feeNames = ['Bidrag til Garantifonden', 'Naturskadeafgift', 'Stormflod og oversvømmelse'];
  return taxesAndFees.some(item => feeNames.includes(item.taxFeeType ?? ''));
};

export const isUnevenPeriod = (invoice: any): boolean => {
  const isStartDateFirstDay = invoice.invoiceOverview.insurances.some(
    (ins: any) => ins.startDate?.day === 1,
  );
  if (isStartDateFirstDay) return false;
  const { standardPriceAfterProration } = invoice.invoiceOverview;
  if (standardPriceAfterProration?.amount) {
    return invoice.invoiceOverview.insurances.length > 0 && standardPriceAfterProration.amount > 0;
  }
  return false;
};

const isCurrentYearAndJanuary = (billDate: GWDate) => {
  const currentYear = new Date().getFullYear();
  return billDate.month === 1 && billDate.year === currentYear;
};

export enum InvoiceType {
  NORMAL='NORMAL', OVERDUE='OVERDUE', EXPIRED='EXPIRED', REMINDER='REMINDER',
  REPLACED='REPLACED', INDEXATION='INDEXATION', TAXATION='TAXATION',
  UNEVENPERIOD='UNEVENPERIOD', REIMBURSEMENT='REIMBURSEMENT',
}

export const getGWInvoiceTypes = (gwInvoice: any): InvoiceType[] => {
  const attrs: InvoiceType[] = [];
  if (isUnevenPeriod(gwInvoice)) attrs.push(InvoiceType.UNEVENPERIOD);
  if (isCurrentYearAndJanuary(gwInvoice.invoiceOverview.billDate)) attrs.push(InvoiceType.INDEXATION);
  if (hasYearlyTaxes(gwInvoice.invoiceOverview.taxesAndFees)) attrs.push(InvoiceType.TAXATION);
  if (['ZeroAmountInvoice','DisbursementToCustomer'].includes(gwInvoice.invoiceOverview.invoiceType)) attrs.push(InvoiceType.REIMBURSEMENT);
  if (gwInvoice.invoiceOverview.isLawyerLetterSent) attrs.push(InvoiceType.EXPIRED);
  if (gwInvoice.invoiceOverview.invoiceReplacedBy) attrs.push(InvoiceType.REPLACED);
  if (!gwInvoice.paymentDetails.isInvoicePaid && validateIfDueDateHasPassedGWDate(gwInvoice.invoiceOverview.dueDate) && !attrs.includes(InvoiceType.REIMBURSEMENT)) attrs.push(InvoiceType.OVERDUE);
  if (gwInvoice.invoiceOverview.taxesAndFees?.find((item: any) => item.taxFeeText.toLowerCase().includes('rykkergebyr'))) attrs.push(InvoiceType.REMINDER);
  if (attrs.length === 0) attrs.push(InvoiceType.NORMAL);
  return attrs;
};

export const extractPeriodFromGWInvoice = (invoice: any): string => {
  if (!invoice) return '';
  if (!invoice.startDate || !invoice.endDate) return '';
  const start = getMomentFromGWDate(invoice.startDate).format('D. MMM. YYYY').toUpperCase();
  const end = getMomentFromGWDate(invoice.endDate).format('D. MMM. YYYY').toUpperCase();
  return `${start} - ${end}`;
};

export const getInsuranceFeesGuidewire = (fees: any[], invoice: any) => {
  const feeEntries: Array<{label: string; value: string}> = [];
  let totalFeeValue = 0;
  fees.forEach((fee: any) => {
    const matchCount = fee.flatFeeDetails.filter((d: any) => d.insuranceName === invoice.insuranceName).length;
    if (matchCount > 0) {
      const perInsurance = Math.round(fee.totalAmount / fee.flatFeeDetails.length);
      feeEntries.push({ label: fee.feeName, value: String(perInsurance) });
      totalFeeValue += perInsurance;
    }
  });
  return { feeEntries, totalFeeValue };
};

export const getMissingPaymentEntriesGWRecursively = (
  allInvoices: any[], currentInvoice: any, entriesFound: any[], t: (key: string) => string
): any[] => {
  if (!currentInvoice.invoiceOverview.fromPriorInvoice) return entriesFound;
  const priorNumber = currentInvoice.invoiceOverview.fromPriorInvoice.priorInvoiceNumber;
  const priorInvoice = allInvoices.find((inv: any) => inv.invoiceOverview.invoiceNumber === priorNumber);
  if (!priorInvoice) return entriesFound;

  const dueDate = priorInvoice.invoiceOverview.dueDate;
  const monthIndex = dueDate?.month;
  const isMonthly = priorInvoice.paymentDetails?.paymentPlan === 'monthly';
  const periodName = isMonthly && monthIndex >= 1 && monthIndex <= 12
    ? danishMonths[monthIndex - 1].toLowerCase()
    : String(dueDate?.year ?? '');

  entriesFound.push({
    invoiceFees: { feeEntries: [], totalFeeValue: 0 },
    invoiceName: `${t('paymentFor')} ${periodName}`,
    invoicePeriod: '',
    invoiceTotalAmount: priorInvoice.invoiceOverview.totalDueAmount?.amount ?? 0,
    invoiceIsRefund: false,
  });

  return entriesFound;
};

export const getPaymentPeriodNameOfNextInvoiceGW = (allInvoices: any[], currentInvoice: any): string | null => {
  if (!currentInvoice.invoiceOverview.invoiceReplacedBy) return null;
  const newerInvoice = allInvoices.find((inv: any) => inv.invoiceOverview.invoiceNumber === currentInvoice.invoiceOverview.invoiceReplacedBy);
  if (!newerInvoice) return null;
  const month = newerInvoice.invoiceOverview.dueDate?.month;
  if (month >= 1 && month <= 12) return danishMonths[month - 1].toLowerCase();
  return null;
};

export const resolveIfInvoiceConvertedToYearlyPaymentGW = (allInvoices: any[], invoiceToCheck: any): boolean => {
  if (invoiceToCheck.paymentDetails?.paymentPlan !== 'yearly') return false;
  if (!invoiceToCheck.invoiceOverview?.fromPriorInvoice) return false;
  const priorNumber = invoiceToCheck.invoiceOverview.fromPriorInvoice.priorInvoiceNumber;
  const prior = allInvoices.find((inv: any) => inv.invoiceOverview?.invoiceNumber === priorNumber);
  if (!prior) return false;
  return prior.paymentDetails?.paymentPlan === 'monthly';
};

export const populateMapWithGWInvoicePeriods = (
  invoiceOverview: any, periodType: string, map: Map<string, any[]>, t: (k: string) => string, _id: string,
) => {
  const gwInvoices = invoiceOverview.guidewireInvoices ?? [];
  for (const inv of gwInvoices) {
    const dueDate = inv.invoiceOverview?.dueDate;
    if (!dueDate) continue;
    const date = getMomentFromGWDate(dueDate).toDate();
    const key = periodType === 'year' ? `${date.getFullYear()}` : `${danishMonths[date.getMonth()]} ${date.getFullYear()}`;
    if (!map.has(key)) map.set(key, []);
    const entries = map.get(key)!;
    const amount = inv.invoiceOverview?.totalDueAmount?.amount ?? 0;
    entries.push({
      invoiceName: inv.invoiceOverview?.insurances?.[0]?.insuranceName ?? 'Unknown',
      totalPaymentAmount: amount,
      isPaid: inv.paymentDetails?.isInvoicePaid ?? false,
      invoiceTypeAttributes: getGWInvoiceTypes(inv),
      periodInvoices: [{ invoiceIsRefund: amount < 0 }],
    });
  }
};
