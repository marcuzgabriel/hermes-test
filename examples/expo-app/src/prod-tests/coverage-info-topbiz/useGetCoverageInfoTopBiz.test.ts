import { test, group, beforeEach, renderHook, mockModule, expect } from 'hermes-test';
// Production test port: useGetCoverageInfoTopBiz — 7 tests
// Original: apps/topdanmark/src/areas/Insurance/Coverage/hooks/__tests__/useGetCoverageInfoTopBiz.test.ts
//
// Hook with mockModule for useGetInsuranceMetaInfo. Tests different TopBiz coverage types.


let metaInfoReturn: any = {};
mockModule('./useGetInsuranceMetaInfo', () => ({
  useGetInsuranceMetaInfo: () => metaInfoReturn,
}));

import { useGetCoverageInfoTopBiz } from './useGetCoverageInfoTopBiz';

const selectedProduct = { system: 'TOP_BIZ', insuranceAgreementNumber: '123', insuranceProductType: 'ALL_RISKS', insuranceProductId: 123 };

const stakeholders = [{ cxrNumber: '38544500', company: { name: 'Ejerforeningen Vestergade', address: { domesticAddress: { streetName: 'Fredensgade 2, A', zipCode: '6600 Vejen' } } }, stakeholderId: '0003785' }];
const insuranceObjectList = [{ turnOver: { operatingLosses: 12, geograficArea: { text: 'Danmark' } }, activityList: [{ mainActivity: true, transportActivityAmount: 159000, typeOfBusiness: { id: '315', text: 'Anlæg af ledningsnet' } }] }];

beforeEach(() => { metaInfoReturn = {}; });

group('useGetCoverageInfoTopBiz', () => {
  test('netbank coverage — default fields', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'ALL_RISKS' };
    const coverage = { coverageType: { category: null, coverageAmount1: 3173824, name: 'Netbank' }, deductibles: [{ deductPercent: '0', deductText: 'Selvrisiko 6.348 kr.', deductAmount: 6348 }], coInsuredStakeholderList: [] };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    expect(current.coverageTitle).toBe('Netbank');
    expect(current.deductibleAmount).toBe('6348');
    expect(current.coverageInfoTitleBundles.length).toBeGreaterThan(0);
  });

  test('liability coverage — variable deductible + co-insured', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'LIABILITY' };
    const coverage = {
      deductibles: [{ deductText: '10% min 5.000 kr.', extendedTerms: 'Extended terms text', deductAmount: 6348, deductPercent: '0' }],
      coverageType: { category: { id: 'PA', text: 'Produktansvar' }, name: 'Produktansvar', coverageAmount1: 10000000, coverageAmount2: 10000000 },
      coInsuredStakeholderList: [
        { stakeholderId: '0621828', company: { address: { domesticAddress: { zipCode: '5500 Middelfart', streetName: 'Damgårdvej 5' } }, name: 'Nordea Finans A/s' }, cxrNumber: '28987234' },
      ],
    };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    expect(current.deductibleTitle).toBe('deductibleVariableShort');
    expect(current.deductibleDescription).toBe('Extended terms text');
    // Title === category ("Produktansvar") so subtitle is empty (production logic)
    expect(current.coverageSubtitle).toBe('');
    // Co-insured stakeholders listed
    const securedBundle = current.coverageInfoTitleBundles.find((b: any) => b.items.some((i: any) => i.title === 'aboutInsuranceMetaSecured'));
    expect(securedBundle).toBeDefined();
  });

  test('transport coverage — activities', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'TRANSPORT' };
    const coverage = { coverageType: { name: 'Transport', coverageAmount1: 500000 }, deductibles: [{ deductAmount: 5000, deductText: '5000 kr.' }], coInsuredStakeholderList: [] };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    expect(current.deductibleDescription).toBe('coverageAmountPerActivity');
    const actBundle = current.coverageInfoTitleBundles[0];
    expect(actBundle.items[0].title).toBe('Anlæg af ledningsnet');
  });

  test('machine coverage — details list', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'MACHINE' };
    const coverage = {
      coverageType: { name: 'Maskine', coverageAmount1: 100000 },
      deductibles: [{ deductAmount: 2000, deductText: '2000 kr.' }],
      coInsuredStakeholderList: [],
      coverageDetailsList: [{ type: 'machineInsurance', machineType: 'Gravemaskine', identification: 'GRV-001', machineId: '42', sum: 500000 }],
    };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    expect(current.coverageInfoTitleBundles.length).toBeGreaterThan(0);
    expect(current.coverageInfoTitleBundles[0].headerLabel).toContain('42');
  });

  test('business interruption — coverage period', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'BUSINESS_INTERRUPTION' };
    const coverage = { coverageType: { name: 'Driftstab', coverageAmount1: 1000000 }, deductibles: [{ deductAmount: 3000, deductText: '3000 kr.' }], coInsuredStakeholderList: [] };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    const periodBundle = current.coverageInfoTitleBundles.find((b: any) => b.items.some((i: any) => i.title === 'coveragePeriod'));
    expect(periodBundle).toBeDefined();
    expect(periodBundle.items[0].subtitle).toContain('12');
  });

  test('occupational statutory — max per employee + per accident', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'OCCUPATIONAL_STATUTORY' };
    const coverage = { coverageType: { name: 'Arbejdsskade', coverageAmount1: 5000000, coverageAmount2: 10000000 }, deductibles: [{ deductAmount: 1000, deductText: '1000 kr.' }], coInsuredStakeholderList: [] };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    const bundle = current.coverageInfoTitleBundles[0];
    expect(bundle.items.length).toBe(2);
    expect(bundle.items[0].title).toBe('coverageAmountMaxPerEmployeePerAccident');
    expect(bundle.items[1].title).toBe('coverageAmountMaxPerAccident');
  });

  test('movable property — default + coverage details', () => {
    metaInfoReturn = { stakeholders, insuranceObjectList, productType: 'MOVABLE_PROPERTY' };
    const coverage = {
      coverageType: { name: 'Løsøre', coverageAmount1: 2000000 },
      deductibles: [{ deductAmount: 5000, deductText: '5000 kr.' }],
      coInsuredStakeholderList: [],
      coverageDetailsList: [{ type: 'regularInsurance', description: '- Bygning', amount: 800000 }],
    };
    const { current } = renderHook(() => useGetCoverageInfoTopBiz({ coverage, selectedInsuranceProduct: selectedProduct }));
    expect(current.coverageInfoTitleBundles[0].items.length).toBe(2);
    expect(current.coverageInfoTitleBundles[0].items[1].title).toBe('Bygning');
  });
});
