// Production test port: useHasPrivateContentInsurance — 8 tests
// Original: apps/topdanmark/src/hooks/insurances/__tests__/useHasPrivateContentInsurance.test.ts
//
// Zero mocks. Store seeded with products, hook reads via useSelector.

const { test, group, beforeEach, expect } =
  (globalThis as any).__metroTest;

import { withStore } from '../shared/testStore';
import { useHasPrivateContentInsurance } from './useHasPrivateContentInsurances';

// Enums matching production values
const System = { PRIMO: 'PRIMO', TOP_BIZ: 'TOP_BIZ', GUIDEWIRE: 'GUIDEWIRE', UNKNOWN: 'UNKNOWN' };
const PrimoType = { HOUSEHOLD_GOODS: 'HOUSEHOLD_GOODS', CAR_RENTAL: 'CAR_RENTAL', ACCIDENT: 'ACCIDENT', ANNUAL_TRAVEL: 'ANNUAL_TRAVEL' };
const TopBizType = { BUILDING_DAMAGE: 'BUILDING_DAMAGE', BUSINESS_TRAVEL: 'BUSINESS_TRAVEL', BUSINESS_MOVABLE_PROPERTY: 'BUSINESS_MOVABLE_PROPERTY' };
const LobName = { PRIVATE_CONTENT: 'PRIVATE_CONTENT', ACCIDENT: 'ACCIDENT' };
const LobFlavor = { STANDARD: 'STANDARD', COOP_STANDARD: 'COOP_STANDARD', YOUTH: 'YOUTH' };

let ctx: ReturnType<typeof withStore>;

beforeEach(() => {
  ctx = withStore({ insurances: { products: [] } });
});

// Helpers
const product = (system: string, type: string, gw?: any) => ({
  insuranceDetails: { id: '1', text: '', description: '', productType: type, status: null },
  system,
  paymentDetails: { signedUpForPaymentService: true, signedUpForMobilePay: false, status: 'okay' },
  insuranceAgreementNumber: '123', detailsFetchable: true,
  insuranceProductType: type, insuranceProductDisplayTitleKey: type,
  ...(gw ? { guidewireIdentifier: gw } : {}),
});

const gwIdentifier = (lobName: string, lobFlavor: string) => ({
  lobName, lobFlavor, lobFlavorCoverableId: '', shouldFetchDetails: false,
});

// =============================================
group('hasPrivateContentInsurance / privateContentInsurances', () => {
  test('returns false when no privateContent insurances', ({ expect }: any) => {
    ctx.patchState({ insurances: { products: [
      product(System.PRIMO, PrimoType.ACCIDENT),
      product(System.UNKNOWN, TopBizType.BUILDING_DAMAGE),
    ]}});

    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());
    expect(current.hasPrivateContentInsurance).toBe(false);
    expect(current.privateContentInsurances).toEqual([]);
  });

  test('returns true for Primo HOUSEHOLD_GOODS', ({ expect }: any) => {
    const products = [
      product(System.PRIMO, PrimoType.CAR_RENTAL),
      product(System.PRIMO, PrimoType.HOUSEHOLD_GOODS),
    ];
    ctx.patchState({ insurances: { products } });

    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());
    expect(current.hasPrivateContentInsurance).toBe(true);
    expect(current.privateContentInsurances).toEqual([products[1]]);
  });

  test('returns false for TopBiz business insurance', ({ expect }: any) => {
    ctx.patchState({ insurances: { products: [product(System.TOP_BIZ, TopBizType.BUSINESS_TRAVEL)] } });

    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());
    expect(current.hasPrivateContentInsurance).toBe(false);
  });

  test('returns true for Guidewire STANDARD', ({ expect }: any) => {
    const p = [product(System.GUIDEWIRE, PrimoType.HOUSEHOLD_GOODS, gwIdentifier(LobName.PRIVATE_CONTENT, LobFlavor.STANDARD))];
    ctx.patchState({ insurances: { products: p } });

    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());
    expect(current.hasPrivateContentInsurance).toBe(true);
    expect(current.privateContentInsurances).toEqual([p[0]]);
  });

  test('returns true for Guidewire COOP_STANDARD', ({ expect }: any) => {
    ctx.patchState({ insurances: { products: [product(System.GUIDEWIRE, PrimoType.HOUSEHOLD_GOODS, gwIdentifier(LobName.PRIVATE_CONTENT, LobFlavor.COOP_STANDARD))] } });

    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());
    expect(current.hasPrivateContentInsurance).toBe(true);
  });

  test('returns true for Guidewire YOUTH', ({ expect }: any) => {
    ctx.patchState({ insurances: { products: [product(System.GUIDEWIRE, PrimoType.HOUSEHOLD_GOODS, gwIdentifier(LobName.PRIVATE_CONTENT, LobFlavor.YOUTH))] } });

    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());
    expect(current.hasPrivateContentInsurance).toBe(true);
  });
});

// =============================================
group('isPrivateContentInsurance', () => {
  test('returns true for valid privateContent selections', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());

    expect(current.isPrivateContentInsurance({ system: System.PRIMO, insuranceProductType: PrimoType.HOUSEHOLD_GOODS })).toBe(true);
    expect(current.isPrivateContentInsurance({ system: System.TOP_BIZ, insuranceProductType: TopBizType.BUSINESS_MOVABLE_PROPERTY })).toBe(true);
    expect(current.isPrivateContentInsurance({ system: System.GUIDEWIRE, guidewireIdentifier: gwIdentifier(LobName.PRIVATE_CONTENT, LobFlavor.STANDARD) })).toBe(true);
    expect(current.isPrivateContentInsurance({ system: System.GUIDEWIRE, guidewireIdentifier: gwIdentifier(LobName.PRIVATE_CONTENT, LobFlavor.COOP_STANDARD) })).toBe(true);
    expect(current.isPrivateContentInsurance({ system: System.GUIDEWIRE, guidewireIdentifier: gwIdentifier(LobName.PRIVATE_CONTENT, LobFlavor.YOUTH) })).toBe(true);
  });

  test('returns false for non-privateContent selections', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useHasPrivateContentInsurance());

    expect(current.isPrivateContentInsurance({ system: System.PRIMO, insuranceProductType: PrimoType.ACCIDENT })).toBe(false);
    expect(current.isPrivateContentInsurance({ system: System.TOP_BIZ, insuranceProductType: TopBizType.BUILDING_DAMAGE })).toBe(false);
    expect(current.isPrivateContentInsurance({ system: System.GUIDEWIRE, guidewireIdentifier: gwIdentifier(LobName.ACCIDENT, 'INDIVIDUAL') })).toBe(false);
    expect(current.isPrivateContentInsurance({ system: System.UNKNOWN, insuranceProductType: PrimoType.ANNUAL_TRAVEL })).toBe(false);
  });
});
