// Production test port: useGetCoverageInfoGuidewire — 6 tests
const { test, group, renderHook, expect } = (globalThis as any).__HT;
import { useGetCoverageInfoGuidewire } from './useGetCoverageInfoGuidewire';

const baseCoverage = { publicID: 'cov1', name: 'Varigt mén og tandskade' };
const baseCoverageData = [{ coverageItemId: 'cov1', title: 'Varigt mén og tandskade', subtitle: 'Subtitle text', included: { title: 'Dækker', items: ['Item 1'] }, excluded: { title: 'Dækker ikke', items: ['Item 2'] } }];

group('useGetCoverageInfoGuidewire', () => {
  test('returns title and subtitle from CMS data', ({ expect }: any) => {
    const { current } = renderHook(() => useGetCoverageInfoGuidewire({
      insuranceDetails: { coverages: [baseCoverage], terms: [] }, coverageData: baseCoverageData, lobName: 'accident', selectedInsuranceProduct: {},
    }));
    expect(current.coverageTitle).toBe('Varigt mén og tandskade');
    expect(current.coverageSubtitle).toBe('Subtitle text');
  });

  test('returns deductibleNone when no deductible terms', ({ expect }: any) => {
    const { current } = renderHook(() => useGetCoverageInfoGuidewire({
      insuranceDetails: { coverages: [baseCoverage], terms: [] }, coverageData: baseCoverageData, lobName: 'accident', selectedInsuranceProduct: {},
    }));
    expect(current.deductibles.textForDisplay).toBe('deductibleNone');
    expect(current.deductibles.deductAmount).toBe('');
  });

  test('returns deductible amount for money type', ({ expect }: any) => {
    const terms = [{ modelType_Top: 'Deductible', type: 'FixedDeductible', name: 'Selvrisiko', chosenTermValue: '750', valueTypeCode_Top: 'money' }];
    const { current } = renderHook(() => useGetCoverageInfoGuidewire({
      insuranceDetails: { coverages: [baseCoverage], terms }, coverageData: baseCoverageData, lobName: 'accident', selectedInsuranceProduct: {},
    }));
    expect(current.deductibles.deductAmount).toBe('750');
    expect(current.deductibles.textForDisplay).toContain('750');
  });

  test('returns insured sums from terms', ({ expect }: any) => {
    const terms = [{ modelType_Top: 'InsuredSum', name: 'Forsikringssum', chosenTermValue: '1000000', valueTypeCode_Top: 'money' }];
    const { current } = renderHook(() => useGetCoverageInfoGuidewire({
      insuranceDetails: { coverages: [baseCoverage], terms }, coverageData: baseCoverageData, lobName: 'accident', selectedInsuranceProduct: {},
    }));
    expect(current.coverageInsuredSums.length).toBe(1);
    expect(current.coverageInsuredSums[0].title).toBe('Forsikringssum');
    expect(current.coverageInsuredSums[0].content).toContain('1000000');
  });

  test('returns coverage details from CMS', ({ expect }: any) => {
    const { current } = renderHook(() => useGetCoverageInfoGuidewire({
      insuranceDetails: { coverages: [baseCoverage], terms: [] }, coverageData: baseCoverageData, lobName: 'accident', selectedInsuranceProduct: {},
    }));
    expect(current.coverageDetails.included.items.length).toBe(1);
    expect(current.coverageDetails.excluded.items.length).toBe(1);
  });

  test('returns extend link for travel', ({ expect }: any) => {
    const { current } = renderHook(() => useGetCoverageInfoGuidewire({
      insuranceDetails: { coverages: [baseCoverage], terms: [] }, coverageData: baseCoverageData, lobName: 'travel', selectedInsuranceProduct: { insuranceAgreementNumber: 'AG999' },
    }));
    expect(current.extendCoverageLink).toContain('travel/extend/AG999');
  });
});
