// Production test port: useGetUserInformation — 22 tests
// Original: apps/topdanmark/src/hooks/userInformation/__tests__/useGetUserInformation.test.ts
//
// Hook reads user data from store, derives hasEmail/hasPhone/isBusinessLogin etc.
// Tests keyValueStorage side effect, stakeholder views, business logic.

const { test, group, beforeEach, spy, expect } = (globalThis as any).__HT;
import { withStore } from '../shared/testStore';
import { useGetUserInformation, keyValueStorage } from './useGetUserInformation';

const mockUser = {
  partyId: '123456789', firstName: 'test', lastName: 'test', fullName: 'test test',
  initials: 'tt', isCompany: false, phoneNumber: '12345678', mobilePhone: '12345678',
  emailAddress: 'test@tester.dk', formattedDomesticAddress: 'test',
  domesticAddress: { zipCode: '9382', subAddress: null, streetName: 'Dunbirkevej', postBox: null, houseNumber: '6', floor: null, door: '', coName: null, city: 'Tylstrup' },
};

let ctx: ReturnType<typeof withStore>;
const removeErrorMock = spy(() => {});
const dispatchMock = spy(() => {});

beforeEach(() => {
  keyValueStorage.reset();
  removeErrorMock.reset();
  dispatchMock.reset();
  ctx = withStore({ user: { information: undefined, stakeholderView: null, stakeholderViews: [], _dispatchMock: dispatchMock, _removeErrorMock: removeErrorMock } });
});

group('useGetUserInformation', () => {
  test('returns initial state', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.information).toBeUndefined();
  });

  test('returns user information', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: null, stakeholderViews: [] } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.information).toEqual(mockUser);
  });

  test('fetchUserInformation calls dispatch', ({ expect }: any) => {
    ctx.patchState({ user: { information: undefined, stakeholderView: null, _dispatchMock: dispatchMock } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    current.fetchUserInformation();
    expect(dispatchMock).wasCalled();
  });

  test('stores name to keyValueStorage', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: null } });
    ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(keyValueStorage.get('STAKEHOLDER_NAME')).toEqual({ firstName: 'test', fullName: 'test test' });
  });

  test('stores company name to keyValueStorage', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, isCompany: true }, stakeholderView: null } });
    ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(keyValueStorage.get('STAKEHOLDER_NAME')).toEqual({ firstName: 'test test', fullName: 'test test' });
  });

  test('hasEmail returns null if no information', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasEmail).toBeNull();
  });

  test('hasEmail returns true if email set', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasEmail).toBe(true);
  });

  test('hasEmail returns false if email null', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, emailAddress: null }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasEmail).toBe(false);
  });

  test('hasEmail returns false if email empty', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, emailAddress: '' }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasEmail).toBe(false);
  });

  test('hasPhone returns null if no information', ({ expect }: any) => {
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasPhone).toBeNull();
  });

  test('hasPhone returns true if phone set', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasPhone).toBe(true);
  });

  test('hasPhone returns true if only phoneNumber set', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, mobilePhone: null }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasPhone).toBe(true);
  });

  test('hasPhone returns false if both null', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, mobilePhone: null, phoneNumber: null }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasPhone).toBe(false);
  });

  test('hasPhone returns false if both empty', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, mobilePhone: null, phoneNumber: '' }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasPhone).toBe(false);
  });

  test('hasPhone returns false if mobilePhone empty', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, mobilePhone: '', phoneNumber: null }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.hasPhone).toBe(false);
  });

  test('returns formattedAddress', ({ expect }: any) => {
    ctx.patchState({ user: { information: { ...mockUser, formattedDomesticAddress: 'Dunbirkevej 6, 9382 Tylstrup' }, stakeholderView: null } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.information?.formattedDomesticAddress).toBe('Dunbirkevej 6, 9382 Tylstrup');
  });

  test('returns stakeholderView', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: { id: '123', name: 'Børge', type: 'household' } } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.stakeholderView).toEqual({ id: '123', name: 'Børge', type: 'household' });
  });

  test('dismissError calls removeError', ({ expect }: any) => {
    ctx.patchState({ user: { information: undefined, _removeErrorMock: removeErrorMock } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    current.dismissError();
    expect(removeErrorMock).wasCalledWith('getStakeholderInfo');
  });

  test('isTotalHouseholdNumberHigherThanOne', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: { totalHouseholdMember: 2 } } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.isTotalHouseholdNumberHigherThanOne).toBe(true);
  });

  test('isBusinessLogin and not isBusinessPOA', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: { type: 'business' }, stakeholderViews: [] } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.isBusinessLogin).toBe(true);
    expect(current.isBusinessPOA).toBe(false);
  });

  test('isBusinessPOA and not isBusinessLogin', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: { type: 'business' }, stakeholderViews: [{ type: 'household' }, { type: 'business' }] } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.isBusinessLogin).toBe(false);
    expect(current.isBusinessPOA).toBe(true);
  });

  test('neither isBusinessPOA nor isBusinessLogin', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: { type: 'household' }, stakeholderViews: [{ type: 'private' }] } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.isBusinessLogin).toBe(false);
    expect(current.isBusinessPOA).toBe(false);
  });

  test('returns mainStakeholder', ({ expect }: any) => {
    ctx.patchState({ user: { information: mockUser, stakeholderView: { type: 'household' }, stakeholderViews: [{ type: 'household', id: '123', name: 'Børge' }] } });
    const { current } = ctx.renderHookWithReduxStore(() => useGetUserInformation());
    expect(current.mainStakeholder).toEqual({ type: 'household', id: '123', name: 'Børge' });
  });
});
