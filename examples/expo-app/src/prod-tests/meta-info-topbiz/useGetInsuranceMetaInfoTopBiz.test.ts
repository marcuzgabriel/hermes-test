// Production test port: useGetInsuranceMetaInfoTopBiz — 5 tests
const { test, group, beforeEach, expect } = (globalThis as any).__HT;
import { withStore } from '../shared/testStore';
import { useGetInsuranceMetaInfoTopBiz } from './useGetInsuranceMetaInfoTopBiz';

let ctx: ReturnType<typeof withStore>;
beforeEach(() => { ctx = withStore({ insuranceDetails: { topBiz: null } }); });

const stakeholders = [{ cxrNumber: '38544500', company: { name: 'TestCompany', address: { domesticAddress: { streetName: 'Street 1', zipCode: '1234' } } } }];
const objectList = [{ turnOver: { operatingLosses: 12 }, activityList: [{ mainActivity: true, transportActivityAmount: 159000, typeOfBusiness: { text: 'Anlæg' } }] }];

group('useGetInsuranceMetaInfoTopBiz', () => {
  test('returns null when no details', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoTopBiz({ insuranceProductType: 'LIABILITY' }));
    expect(current).toBeNull();
  });

  test('returns stakeholders and product type', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { topBiz: { stakeholders, insuranceObjectList: objectList, coverages: [] } } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoTopBiz({ insuranceProductType: 'LIABILITY' }));
    expect(current.productType).toBe('LIABILITY');
    expect(current.stakeholders.length).toBe(1);
    expect(current.stakeholders[0].company.name).toBe('TestCompany');
  });

  test('returns activities from insurance object list', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { topBiz: { stakeholders, insuranceObjectList: objectList, coverages: [] } } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoTopBiz({ insuranceProductType: 'TRANSPORT' }));
    expect(current.activities.length).toBe(1);
    expect(current.activities[0].typeOfBusiness.text).toBe('Anlæg');
  });

  test('returns operating losses', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { topBiz: { stakeholders, insuranceObjectList: objectList, coverages: [] } } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoTopBiz({ insuranceProductType: 'BUSINESS_INTERRUPTION' }));
    expect(current.operatingLosses).toBe(12);
  });

  test('returns coverages', ({ expect }: any) => {
    const coverages = [{ name: 'Ansvar', coverageAmount: 5000000 }];
    ctx.patchState({ insuranceDetails: { topBiz: { stakeholders, insuranceObjectList: objectList, coverages } } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoTopBiz({ insuranceProductType: 'LIABILITY' }));
    expect(current.coverages.length).toBe(1);
    expect(current.coverages[0].name).toBe('Ansvar');
  });
});
