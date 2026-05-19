// Real production code — Primo invoice utilities
import moment from 'moment-timezone';

export enum InvoiceType { NORMAL='NORMAL', OVERDUE='OVERDUE', REMINDER='REMINDER', CANCELLED='CANCELLED' }

export const getPrimoInvoiceTypes = (invoice: any): InvoiceType[] => {
  const attrs: InvoiceType[] = [];
  if (invoice.invoiceStatus === 'Annulleret') attrs.push(InvoiceType.CANCELLED);
  if (invoice.invoiceReminderDate) attrs.push(InvoiceType.REMINDER);
  const dueDate = new Date(invoice.invoiceDueDate);
  if (!invoice.invoicePaidIndicator && dueDate < new Date()) attrs.push(InvoiceType.OVERDUE);
  if (attrs.length === 0) attrs.push(InvoiceType.NORMAL);
  return attrs;
};

export const extractPeriodFromPrimoInvoice = (details: any): string => {
  if (!details) return '';
  const specs = details.paymentSpecifications ?? [];
  const match = specs.find((s: any) => s.specificationLabel?.includes('perioden'));
  if (!match?.specificationLabel) return '';
  const periodPart = match.specificationLabel.split('perioden ')[1];
  if (!periodPart) return '';
  return periodPart.replace(/\b(januar|februar|marts|april|maj|juni|juli|august|september|oktober|november|december)\b/gi, (m: string) => {
    const abbrev = m.substring(0, 3);
    return abbrev.toUpperCase() + '.';
  }).toUpperCase();
};

export const getInsuranceAttachedTopFeesFromInvoiceDetailsPrimo = (detail: any) => {
  const fees: Array<{label: string; value: string}> = [];
  const topFeeLabels = ['Opkrævningsgebyr', 'Stempelafgift', 'Stormflodsafgift', 'Rykkergebyr'];
  for (const spec of (detail.paymentSpecifications ?? [])) {
    if (topFeeLabels.includes(spec.specificationLabel)) {
      fees.push({ label: spec.specificationLabel, value: String(spec.specificationValue ?? 0) });
    }
  }
  return fees;
};

export const getAllInsuranceTopLevelFeesPrimo = (invoice: any) => {
  const allFees: Array<{label: string; value: string}> = [];
  for (const detail of (invoice.invoiceDetails ?? [])) {
    const fees = getInsuranceAttachedTopFeesFromInvoiceDetailsPrimo(detail);
    for (const fee of fees) {
      const existing = allFees.find(f => f.label === fee.label);
      if (existing) existing.value = String(Number(existing.value) + Number(fee.value));
      else allFees.push({ ...fee });
    }
  }
  return allFees;
};

export const getInsuranceAttachedFeesPrimo = (detail: any) => {
  const fees: Array<{label: string; value: string}> = [];
  const feeLabels = ['Forsikringsafgift', 'Garantifondsafgift'];
  for (const spec of (detail.paymentSpecifications ?? [])) {
    if (feeLabels.includes(spec.specificationLabel)) {
      fees.push({ label: spec.specificationLabel, value: String(spec.specificationValue ?? 0) });
    }
  }
  return fees;
};

export const getAllMissingInvoicePaymentEntriesPrimo = (invoice: any) => {
  const entries: any[] = [];
  for (const detail of (invoice.invoiceDetails ?? [])) {
    for (const spec of (detail.paymentSpecifications ?? [])) {
      if (spec.specificationType === 'TransferredPrior') {
        entries.push({ invoiceName: spec.specificationLabel, invoiceTotalAmount: spec.specificationValue ?? 0, invoiceIsRefund: (spec.specificationValue ?? 0) < 0 });
      }
    }
  }
  return entries;
};

export const populateMapWithPrimoInvoicePeriods = (invoiceOverview: any, periodType: string, map: Map<string, any[]>, t: (k: string) => string) => {
  for (const inv of (invoiceOverview.primoInvoices ?? [])) {
    const dueDate = new Date(inv.invoiceDueDate);
    const danishMonths = ['Januar','Februar','Marts','April','Maj','Juni','Juli','August','September','Oktober','November','December'];
    const key = periodType === 'year' ? `${dueDate.getFullYear()}` : `${danishMonths[dueDate.getMonth()]} ${dueDate.getFullYear()}`;
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push({ invoiceName: inv.insuranceName ?? 'Unknown', totalPaymentAmount: inv.invoiceTotalAmount ?? 0, isPaid: inv.invoicePaidIndicator ?? false, invoiceTypeAttributes: getPrimoInvoiceTypes(inv), periodInvoices: [] });
  }
};
