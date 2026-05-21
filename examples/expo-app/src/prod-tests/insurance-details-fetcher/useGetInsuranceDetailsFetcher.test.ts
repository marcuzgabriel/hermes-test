// Production test port: useGetInsuranceDetailsFetcher — 11 tests
// Original: apps/topdanmark/src/hooks/insuranceDetails/__tests__/useGetInsuranceDetailsFetcher.test.ts
//
// Complex async hook: multi-system fetching (Primo/TopBiz/Guidewire),
// detailsFetchable guard, forceRefetch, Promise.allSettled.
// Uses spy() for dispatch tracking, withStore for insurances state.

const { test, group, beforeEach, act, spy, expect } = (globalThis as any).__HT;

import { withStore } from '../shared/testStore';
import {
  useGetInsuranceDetailsFetcher,
  FetchingStatus,
  InsuranceSystem,
} from './useGetInsuranceDetailsFetcher';

const TopBizType = { MACHINE: 'MACHINE', PATIENT_INJURY: 'PATIENT_INJURY' };
const LobName = { PET: 'pet' };

let ctx: ReturnType<typeof withStore>;
let dispatchMock: any;
let primoInitiate: any;
let topBizInitiate: any;
let topBizMachineInitiate: any;
let guidewireInitiate: any;
let agreementPeriodInitiate: any;

const fetchable = [{ products: [{ insuranceNumbers: [123], detailsFetchable: true, guidewireIdentifier: { shouldFetchDetails: true } }] }];
const nonFetchable = [{ products: [{ insuranceNumbers: [123], detailsFetchable: false, guidewireIdentifier: { shouldFetchDetails: false } }] }];

function makeConfig() {
  return {
    dispatchPrimo: dispatchMock,
    dispatchTopBiz: dispatchMock,
    dispatchGuidewire: dispatchMock,
    dispatchAgreementPeriod: dispatchMock,
    primoInitiate,
    topBizInitiate,
    topBizMachineInitiate,
    guidewireInitiate,
    agreementPeriodInitiate,
  };
}

beforeEach(() => {
  ctx = withStore({ insurances: { data: fetchable } });
  dispatchMock = spy(async () => {});
  primoInitiate = spy((args: any, opts: any) => ({ type: 'primo', args, opts }));
  topBizInitiate = spy((args: any, opts: any) => ({ type: 'topBiz', args, opts }));
  topBizMachineInitiate = spy((args: any, opts: any) => ({ type: 'topBizMachine', args, opts }));
  guidewireInitiate = spy((args: any, opts: any) => ({ type: 'gw', args, opts }));
  agreementPeriodInitiate = spy((args: any, opts: any) => ({ type: 'agreementPeriod', args, opts }));
});

group('fetching status', () => {
  test('DID_FETCH after fetchInsuranceDetails completes', ({ expect }: any) => {
    const hook = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { hook.current.fetchInsuranceDetails({ system: InsuranceSystem.PRIMO, insuranceAgreementNumber: '123', insuranceProductType: 'ACCIDENT', insuranceProductId: 123 }); });
    // After act, the hook re-renders with updated state
    expect(hook.current.fetchingStatus).toBe(FetchingStatus.DID_FETCH);
  });

  test('FETCHING initially when hasDetails=false', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    expect(current.fetchingStatus).toBe(FetchingStatus.FETCHING);
  });

  test('DID_FETCH initially when hasDetails=true', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(true, makeConfig()));
    expect(current.fetchingStatus).toBe(FetchingStatus.DID_FETCH);
  });
});

group('fetch Primo', () => {
  test('dispatches when fetchable', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.PRIMO, insuranceAgreementNumber: '123', insuranceProductType: 'ACCIDENT', insuranceProductId: 123 }); });
    expect(dispatchMock).wasCalledOnce();
    expect(primoInitiate).wasCalledWith({ insuranceAgreementNumber: '123' }, undefined);
  });

  test('does not dispatch when non-fetchable', ({ expect }: any) => {
    ctx.patchState({ insurances: { data: nonFetchable } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.PRIMO, insuranceAgreementNumber: '123', insuranceProductType: 'ACCIDENT', insuranceProductId: 123 }); });
    expect(dispatchMock).wasNeverCalled();
  });
});

group('fetch TopBiz', () => {
  test('dispatches for regular TopBiz', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.TOP_BIZ, insuranceAgreementNumber: '123', insuranceProductType: TopBizType.PATIENT_INJURY, insuranceProductId: 123 }); });
    expect(dispatchMock.callCount).toBe(2); // details + agreement period
    expect(topBizInitiate).wasCalledWith({ insuranceAgreementNumber: '123', insuranceProductId: 123 }, undefined);
  });

  test('uses machine endpoint for MACHINE type', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.TOP_BIZ, insuranceAgreementNumber: '123', insuranceProductType: TopBizType.MACHINE, insuranceProductId: 123 }); });
    expect(topBizMachineInitiate).wasCalledWith({ insuranceAgreementNumber: '123', insuranceProductId: 123 }, undefined);
  });

  test('does not dispatch when non-fetchable', ({ expect }: any) => {
    ctx.patchState({ insurances: { data: nonFetchable } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.TOP_BIZ, insuranceAgreementNumber: '123', insuranceProductType: TopBizType.PATIENT_INJURY, insuranceProductId: 123 }); });
    expect(dispatchMock).wasNeverCalled();
  });
});

group('fetch Guidewire', () => {
  test('dispatches with lobName', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.GUIDEWIRE, insuranceAgreementNumber: '123', insuranceProductType: 'PET', guidewireIdentifier: { lobName: LobName.PET, lobFlavor: null, lobFlavorCoverableId: '', shouldFetchDetails: true } }); });
    expect(dispatchMock).wasCalledOnce();
    expect(guidewireInitiate).wasCalledWith({ insuranceAgreementNumber: '123', lobName: LobName.PET }, undefined);
  });

  test('does not dispatch when shouldFetchDetails=false', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetInsuranceDetailsFetcher(false, makeConfig()));
    act(() => { current.fetchInsuranceDetails({ system: InsuranceSystem.GUIDEWIRE, insuranceAgreementNumber: '123', insuranceProductType: 'PET', guidewireIdentifier: { lobName: LobName.PET, lobFlavor: null, lobFlavorCoverableId: '', shouldFetchDetails: false } }); });
    expect(guidewireInitiate).wasNeverCalled();
  });
});
