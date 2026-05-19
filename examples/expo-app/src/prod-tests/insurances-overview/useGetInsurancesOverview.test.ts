// Production test port: useGetInsurancesOverview — 12 tests
// Original: apps/topdanmark/src/hooks/insurances/__tests__/useGetInsurancesOverview.test.ts
//
// Uses mockModule for getInsuranceTitles + withStore for useGetIndexBySelectedInsuranceProduct.
// Tests the main hook (pure arg), index lookup (store-backed), and product-by-index (store-backed).

const { test, group, beforeEach, renderHook, mockModule, expect } =
  (globalThis as any).__metroTest;

import { withStore } from '../shared/testStore';

// --- Mock getInsuranceTitles ---
let getInsuranceTitlesReturn: any = null;
let getInsuranceTitlesImpl: ((p: any) => any) | null = null;

mockModule('./getInsuranceTitles', () => ({
  getInsuranceTitles: (product: any) =>
    getInsuranceTitlesImpl ? getInsuranceTitlesImpl(product) : getInsuranceTitlesReturn,
}));

import {
  useGetInsurancesOverview,
  useGetIndexBySelectedInsuranceProduct,
  useGetSelectedInsuranceProductByIndex,
} from './useGetInsurancesOverview';

const S = { PRIMO: 'PRIMO', TOP_BIZ: 'TOP_BIZ', GUIDEWIRE: 'GUIDEWIRE' };
const T = { PERSONAL_CAR: 'PERSONAL_CAR', WAGE_INSURANCE: 'WAGE_INSURANCE' };

const mkProduct = (opts: any) => ({
  insuranceDetails: { description: '', id: opts.id ?? '1', text: '', productType: opts.type ?? 'car', status: null },
  system: opts.system ?? S.PRIMO,
  insuranceNumbers: opts.numbers ?? [123],
  paymentDetails: { signedUpForPaymentService: true, signedUpForMobilePay: false, status: 'active' },
  insuranceAgreementNumber: opts.agr ?? 'AG123',
  detailsFetchable: true,
  insuranceProductType: opts.type ?? T.PERSONAL_CAR,
  insuranceProductDisplayTitleKey: opts.displayKey ?? 'personalCar',
  guidewireIdentifier: opts.gw ?? null,
});

let ctx: ReturnType<typeof withStore>;

beforeEach(() => {
  getInsuranceTitlesReturn = null;
  getInsuranceTitlesImpl = null;
  ctx = withStore({ insurances: { products: [] } });
});

// =============================================
group('useGetInsurancesOverview', () => {
  test('returns empty array for empty products', ({ expect }: any) => {
    const { current } = renderHook(() => useGetInsurancesOverview([]));
    expect(current).toEqual([]);
  });

  test('returns overview with titles', ({ expect }: any) => {
    getInsuranceTitlesReturn = { title: 'Car Insurance', subtitle: 'car' };
    const products = [mkProduct({ id: '1', agr: 'AG123', type: T.PERSONAL_CAR, numbers: [123] })];
    const { current } = renderHook(() => useGetInsurancesOverview(products));
    expect(current.length).toBe(1);
    expect(current[0].title).toBe('Car Insurance');
    expect(current[0].subtitle).toBe('car');
    expect(current[0].showExternalLink).toBe(false);
    expect(current[0].selectedInsuranceProduct.insuranceAgreementNumber).toBe('AG123');
  });

  test('handles missing titles gracefully', ({ expect }: any) => {
    getInsuranceTitlesReturn = null;
    const products = [mkProduct({})];
    const { current } = renderHook(() => useGetInsurancesOverview(products));
    expect(current).toEqual([]);
  });

  test('returns showExternalLink for wage insurance', ({ expect }: any) => {
    getInsuranceTitlesReturn = { title: 'Wage Insurance', subtitle: 'wage' };
    const products = [mkProduct({ type: T.WAGE_INSURANCE, displayKey: 'wageInsurance' })];
    const { current } = renderHook(() => useGetInsurancesOverview(products));
    expect(current[0].showExternalLink).toBe(true);
  });
});

// =============================================
group('useGetIndexBySelectedInsuranceProduct', () => {
  const twoProducts = [
    mkProduct({ id: '1', agr: 'AG123', type: T.PERSONAL_CAR, numbers: [123] }),
    mkProduct({ id: '2', agr: 'AG456', type: T.WAGE_INSURANCE, numbers: [456], displayKey: 'wageInsurance' }),
  ];

  function setup() {
    ctx.patchState({ insurances: { products: twoProducts } });
    getInsuranceTitlesImpl = (product: any) => {
      if (product.insuranceProductType === T.PERSONAL_CAR) return { title: 'Car Insurance', subtitle: 'car' };
      if (product.insuranceProductType === T.WAGE_INSURANCE) return { title: 'Wage Insurance', subtitle: 'wage' };
      return null;
    };
  }

  test('returns -1 for null', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetIndexBySelectedInsuranceProduct(null));
    expect(current).toBe(-1);
  });

  test('returns correct index for first product', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetIndexBySelectedInsuranceProduct({
      system: S.PRIMO, insuranceAgreementNumber: 'AG123', insuranceProductType: T.PERSONAL_CAR,
      guidewireIdentifier: null, insuranceProductId: 123, insuranceDetailsId: '1',
    }));
    expect(current).toBe(0);
  });

  test('returns correct index for second product', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetIndexBySelectedInsuranceProduct({
      system: S.PRIMO, insuranceAgreementNumber: 'AG456', insuranceProductType: T.WAGE_INSURANCE,
      guidewireIdentifier: null, insuranceProductId: 456, insuranceDetailsId: '2',
    }));
    expect(current).toBe(1);
  });

  test('returns -1 for non-matching product', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetIndexBySelectedInsuranceProduct({
      system: S.PRIMO, insuranceAgreementNumber: 'AG999', insuranceProductType: T.PERSONAL_CAR,
      guidewireIdentifier: null, insuranceProductId: 999, insuranceDetailsId: '1',
    }));
    expect(current).toBe(-1);
  });
});

// =============================================
group('useGetSelectedInsuranceProductByIndex', () => {
  function setup() {
    ctx.patchState({ insurances: { products: [
      mkProduct({ id: '1', agr: 'AG123', type: T.PERSONAL_CAR, numbers: [123] }),
      mkProduct({ id: '2', agr: 'AG456', type: T.WAGE_INSURANCE, numbers: [456] }),
    ]}});
    getInsuranceTitlesImpl = (product: any) => {
      if (product.insuranceProductType === T.PERSONAL_CAR) return { title: 'Car', subtitle: 'car' };
      if (product.insuranceProductType === T.WAGE_INSURANCE) return { title: 'Wage', subtitle: 'wage' };
      return null;
    };
  }

  test('returns product for valid index', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetSelectedInsuranceProductByIndex(0));
    expect(current.insuranceAgreementNumber).toBe('AG123');
  });

  test('returns product for second index', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetSelectedInsuranceProductByIndex(1));
    expect(current.insuranceAgreementNumber).toBe('AG456');
  });

  test('returns undefined for invalid index', ({ expect }: any) => {
    setup();
    const { current } = ctx.renderHookWithReduxStore(() => useGetSelectedInsuranceProductByIndex(999));
    expect(current).toBeUndefined();
  });
});
