// Production test port: useGetInsuranceMetaInfoGuidewire — 5 tests
const { test, group, beforeEach, expect } = (globalThis as any).__metroTest;
import { withStore } from '../shared/testStore';
import { useGetInsuranceMetaInfoGuidewire } from './useGetInsuranceMetaInfoGuidewire';

let ctx: ReturnType<typeof withStore>;
beforeEach(() => { ctx = withStore({ insuranceDetails: { guidewire: null } }); });

const gwDetails = {
  title: 'Ulykkesforsikring',
  coverages: [{ publicID: 'cov1', name: 'Varigt mén', terms: [] }],
  terms: [
    { modelType_Top: 'InsuredSum', name: 'Forsikringssum', chosenTermValue: '1000000', valueTypeCode_Top: 'money' },
    { modelType_Top: 'Deductible', name: 'Selvrisiko', chosenTermValue: '750', valueTypeCode_Top: 'money', type: 'FixedDeductible' },
  ],
  insuredContentList: [{ category: 'Smykker', sum: 50000 }],
};

group('useGetInsuranceMetaInfoGuidewire', () => {
  test('returns null when no details', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoGuidewire({ guidewireIdentifier: { lobName: 'accident' } }));
    expect(current).toBeNull();
  });

  test('returns coverages and lobName', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { guidewire: gwDetails } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoGuidewire({ guidewireIdentifier: { lobName: 'accident' } }));
    expect(current.lobName).toBe('accident');
    expect(current.coverages.length).toBe(1);
    expect(current.coverages[0].name).toBe('Varigt mén');
  });

  test('returns terms', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { guidewire: gwDetails } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoGuidewire({ guidewireIdentifier: { lobName: 'accident' } }));
    expect(current.terms.length).toBe(2);
    expect(current.terms[0].name).toBe('Forsikringssum');
  });

  test('returns insuredContentList for private content', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { guidewire: gwDetails } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoGuidewire({ guidewireIdentifier: { lobName: 'private_content' } }));
    expect(current.insuredContentList.length).toBe(1);
    expect(current.insuredContentList[0].category).toBe('Smykker');
  });

  test('returns title', ({ expect }: any) => {
    ctx.patchState({ insuranceDetails: { guidewire: gwDetails } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceMetaInfoGuidewire({ guidewireIdentifier: { lobName: 'pet' } }));
    expect(current.title).toBe('Ulykkesforsikring');
  });
});
