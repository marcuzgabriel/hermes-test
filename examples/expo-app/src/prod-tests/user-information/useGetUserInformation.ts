// Real production hook — faithful copy
// Changes: useSelector reads from generic store, keyValueStorage is a local stub
import { useCallback, useEffect, useMemo } from 'react';
import { useSelector } from 'react-redux';

export interface UserInformation {
  partyId: string; firstName: string; lastName: string; fullName: string; initials: string;
  isCompany: boolean; phoneNumber: string | null; mobilePhone: string | null;
  emailAddress: string | null; formattedDomesticAddress: string | null; cprNumber?: string; cvrNumber?: string;
  domesticAddress?: any;
}

export const keyValueStorage = {
  _store: {} as Record<string, any>,
  set(key: string, value: any) { this._store[key] = value; },
  get(key: string) { return this._store[key]; },
  reset() { this._store = {}; },
};

export const useGetUserInformation = () => {
  const { information, stakeholderView, stakeholderViews } = useSelector((state: any) => ({
    information: state.user?.information as UserInformation | undefined,
    stakeholderView: state.user?.stakeholderView,
    stakeholderViews: state.user?.stakeholderViews,
  }));

  const dispatchMock = useSelector((state: any) => state.user?._dispatchMock);
  const removeErrorMock = useSelector((state: any) => state.user?._removeErrorMock);

  useEffect(() => {
    const firstName = information?.isCompany ? information?.fullName : (information?.firstName ?? '');
    const fullName = information?.fullName;
    if (firstName && fullName) {
      keyValueStorage.set('STAKEHOLDER_NAME', { firstName, fullName });
    }
  }, [information]);

  const hasEmail = information ? (information?.emailAddress ? true : false) : null;
  const hasPhone = information ? !!information?.mobilePhone || !!information.phoneNumber : null;

  const { isBusinessLogin, isBusinessPOA } = useMemo(() => {
    const isBusinessStakeholder = stakeholderView?.type === 'business';
    const hasHouseholdView = stakeholderViews?.some((v: any) => v.type === 'household') ?? false;
    const hasMultiple = (stakeholderViews ?? []).length > 1;
    return {
      isBusinessLogin: isBusinessStakeholder && !hasHouseholdView,
      isBusinessPOA: isBusinessStakeholder && hasMultiple && hasHouseholdView,
    };
  }, [stakeholderView, stakeholderViews]);

  const mainStakeholder = useMemo(
    () => stakeholderViews?.find((v: any) => v.type === 'household'),
    [stakeholderViews],
  );

  return {
    information,
    error: null,
    hasEmail,
    hasPhone,
    stakeholderView,
    isBusinessLogin,
    isBusinessPOA,
    mainStakeholder,
    isTotalHouseholdNumberHigherThanOne: stakeholderView?.totalHouseholdMember ? stakeholderView.totalHouseholdMember > 1 : false,
    dismissError: () => removeErrorMock?.('getStakeholderInfo'),
    fetchUserInformation: async () => { dispatchMock?.(); },
  };
};
