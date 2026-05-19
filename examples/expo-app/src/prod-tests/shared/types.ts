// Shared type definitions extracted from the insurance app
// These are the enums and interfaces used across multiple test files

export enum InsuranceSystem {
  PRIMO = 'PRIMO',
  TOP_BIZ = 'TOP_BIZ',
  GUIDEWIRE = 'GUIDEWIRE',
  UNKNOWN = 'UNKNOWN',
}

export enum InsuranceProductTypePrimo {
  PERSONAL_CAR = 'PERSONAL_CAR',
  HOUSEHOLD_GOODS = 'HOUSEHOLD_GOODS',
  HOUSEHOLD_GOODS_NO_LEGAL_OR_THEFT = 'HOUSEHOLD_GOODS_NO_LEGAL_OR_THEFT',
  VACATION_HOME_HOUSEHOLD_GOODS = 'VACATION_HOME_HOUSEHOLD_GOODS',
  ACCIDENT = 'ACCIDENT',
  ANNUAL_TRAVEL = 'ANNUAL_TRAVEL',
  CAR_RENTAL = 'CAR_RENTAL',
  WAGE_INSURANCE = 'WAGE_INSURANCE',
  HOUSE = 'HOUSE',
  MOTORCYCLE = 'MOTORCYCLE',
  DOG = 'DOG',
  CAT = 'CAT',
  BOAT = 'BOAT',
}

export enum InsuranceProductTypeTopBiz {
  BUILDING_DAMAGE = 'BUILDING_DAMAGE',
  BUSINESS_TRAVEL = 'BUSINESS_TRAVEL',
  BUSINESS_MOVABLE_PROPERTY = 'BUSINESS_MOVABLE_PROPERTY',
  LIABILITY = 'LIABILITY',
  OPERATION_LOSS = 'OPERATION_LOSS',
  TRANSPORT = 'TRANSPORT',
  MACHINE = 'MACHINE',
  OCCUPATIONAL_STATUTORY = 'OCCUPATIONAL_STATUTORY',
}

export enum LobName {
  PRIVATE_CONTENT = 'PRIVATE_CONTENT',
  ACCIDENT = 'ACCIDENT',
  PET = 'PET',
  TRAVEL = 'TRAVEL',
  HOUSE = 'HOUSE',
  CAR = 'CAR',
}

export enum LobFlavorPrivateContent {
  STANDARD = 'STANDARD',
  COOP_STANDARD = 'COOP_STANDARD',
  YOUTH = 'YOUTH',
}

export enum LobFlavorTravel {
  INDIVIDUAL = 'INDIVIDUAL',
  FAMILY = 'FAMILY',
}

export interface GuidewireIdentifier {
  lobName: LobName;
  lobFlavor: string;
  lobFlavorCoverableId: string;
  shouldFetchDetails: boolean;
}

export interface InsuranceDetails {
  id: string;
  text: string;
  description: string;
  productType: string;
  status: string | null;
  startDate?: string;
}

export interface PaymentDetails {
  signedUpForPaymentService: boolean;
  signedUpForMobilePay: boolean;
  status: string;
}

export interface InsuranceProduct {
  insuranceDetails: InsuranceDetails;
  system: InsuranceSystem;
  paymentDetails: PaymentDetails;
  insuranceAgreementNumber: string;
  detailsFetchable: boolean;
  insuranceProductType: string;
  insuranceProductDisplayTitleKey: string;
  insuranceNumbers?: number[];
  guidewireIdentifier?: GuidewireIdentifier | null;
}

export interface SelectedInsuranceProduct {
  system: InsuranceSystem;
  insuranceAgreementNumber: string;
  insuranceProductType: string;
  guidewireIdentifier?: GuidewireIdentifier | null;
  insuranceProductId?: number;
  insuranceDetailsId?: string | null;
}

export interface InsuranceOverview {
  selectedInsuranceProduct: SelectedInsuranceProduct;
  title: string;
  subtitle: string;
  showExternalLink: boolean;
  futureEffectiveDate?: string;
}

// Payment types
export enum InvoiceType {
  NORMAL = 'NORMAL',
  OVERDUE = 'OVERDUE',
  EXPIRED = 'EXPIRED',
  REMINDER = 'REMINDER',
  REPLACED = 'REPLACED',
  INDEXATION = 'INDEXATION',
  TAXATION = 'TAXATION',
  UNEVENPERIOD = 'UNEVENPERIOD',
  REIMBURSEMENT = 'REIMBURSEMENT',
  CANCELLED = 'CANCELLED',
}

export enum PaymentType {
  GIRO = 'GIRO',
  PAYMENT_SERVICE = 'PAYMENT_SERVICE',
  MOBILE_PAY = 'MOBILE_PAY',
}

export enum PaymentSystem {
  PRIMO = 'PRIMO',
  TOP_PRO = 'TOP_PRO',
  GUIDEWIRE = 'GUIDEWIRE',
}

export enum TopLevelFeeNameAddedToInvoice {
  REMINDER = 'Rykkergebyr',
  MOBILE_PAY = 'MobilePay gebyr',
}

export interface SimplifiedInvoice {
  invoiceName: string;
  invoicePeriod: string;
  invoiceTotalAmount: number;
  invoiceIsRefund: boolean;
  invoiceFees: {
    feeEntries: Array<{ label: string; value: string }>;
    totalFeeValue: number;
  };
}

export interface InvoicePeriodEntry {
  invoiceName?: string;
  invoicePeriod?: string;
  invoiceTotalAmount?: number;
  invoiceIsRefund?: boolean;
  invoiceFees?: any;
  invoiceTypes?: InvoiceType[];
  paymentMethod?: PaymentType;
  paymentDate?: string;
  isPaid?: boolean;
  [key: string]: any;
}

export interface InvoiceOverview {
  guidewireInvoices: any[];
  primoInvoices: any[];
  topproInvoices: any[];
}

// Danish months lookup used by payment utils
export const danishMonths = [
  'januar', 'februar', 'marts', 'april', 'maj', 'juni',
  'juli', 'august', 'september', 'oktober', 'november', 'december',
];
