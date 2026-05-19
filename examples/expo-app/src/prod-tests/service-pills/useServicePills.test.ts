// Production test port: useServicePills — 8 tests
const { test, group, beforeEach, renderHook, expect } = (globalThis as any).__metroTest;
import { withStore } from '../shared/testStore';
import { useServicePills, pillsFromCoverages, pillsFromInsurances } from './useServicePills';

let ctx: ReturnType<typeof withStore>;
const t = (s: string) => s;
beforeEach(() => { ctx = withStore({ insurance: { coverages: [], products: [] } }); });

group('pillsFromCoverages', () => {
  test('returns carglass pill for glass coverage', ({ expect }: any) => {
    const pills = pillsFromCoverages([{ coverageId: 'carglass-plus' }], t);
    expect(pills.length).toBe(1);
    expect(pills[0].type).toBe('carglass');
  });
  test('returns carwash pill for vask coverage', ({ expect }: any) => {
    const pills = pillsFromCoverages([{ coverageId: 'auto-vask' }], t);
    expect(pills.length).toBe(1);
    expect(pills[0].type).toBe('carwash');
  });
  test('returns roadside pill for vejhjælp coverage', ({ expect }: any) => {
    const pills = pillsFromCoverages([{ coverageId: 'vejhjælp-udvidet' }], t);
    expect(pills.length).toBe(1);
    expect(pills[0].type).toBe('roadsideAssistance');
  });
  test('returns empty for no matching coverages', ({ expect }: any) => {
    const pills = pillsFromCoverages([{ coverageId: 'random-cov' }], t);
    expect(pills.length).toBe(0);
  });
});

group('pillsFromInsurances', () => {
  test('returns carglass pill for car insurance', ({ expect }: any) => {
    const pills = pillsFromInsurances([{ insuranceProductType: 'PERSONAL_CAR' }], 0, t);
    expect(pills.length).toBe(1);
    expect(pills[0].type).toBe('carglass');
  });
  test('deduplicates same type', ({ expect }: any) => {
    const pills = pillsFromInsurances([{ insuranceProductType: 'PERSONAL_CAR' }, { insuranceProductType: 'CAR_RENTAL' }], 0, t);
    expect(pills.length).toBe(1);
  });
});

group('useServicePills hook', () => {
  test('returns combined pills from store', ({ expect }: any) => {
    ctx.patchState({ insurance: { coverages: [{ coverageId: 'carglass-basic' }], products: [] } });
    const { current } = ctx.renderHookWithReduxStore(() => useServicePills());
    expect(current.pills.length).toBe(1);
  });
  test('returns empty for empty store', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useServicePills());
    expect(current.pills.length).toBe(0);
  });
});
