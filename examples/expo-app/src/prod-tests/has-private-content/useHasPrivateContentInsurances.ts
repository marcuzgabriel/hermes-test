// Real production hook — faithful copy from insurance app
// Only change: useSelector reads from state.insurances.products
// (in prod this goes through useGetInsurances → useAppSelector → RTK Query cache)
import { useCallback, useMemo } from 'react';
import { useSelector } from 'react-redux';

enum InsuranceSystem { PRIMO = 'PRIMO', TOP_BIZ = 'TOP_BIZ', GUIDEWIRE = 'GUIDEWIRE', UNKNOWN = 'UNKNOWN' }
enum InsuranceProductTypePrimo { HOUSEHOLD_GOODS = 'HOUSEHOLD_GOODS', HOUSEHOLD_GOODS_NO_LEGAL_OR_THEFT = 'HOUSEHOLD_GOODS_NO_LEGAL_OR_THEFT', VACATION_HOME_HOUSEHOLD_GOODS = 'VACATION_HOME_HOUSEHOLD_GOODS' }
enum InsuranceProductTypeTopBiz { BUSINESS_MOVABLE_PROPERTY = 'BUSINESS_MOVABLE_PROPERTY' }
enum LobName { PRIVATE_CONTENT = 'PRIVATE_CONTENT' }

export const useHasPrivateContentInsurance = () => {
  // In prod: useGetInsurances().insuranceProducts
  // Here: straight from state — same data, same selector pattern
  const insuranceProducts: any[] | null = useSelector((s: any) => s.insurances?.products ?? null);

  const isPrimo = (t: string) => [
    InsuranceProductTypePrimo.HOUSEHOLD_GOODS,
    InsuranceProductTypePrimo.HOUSEHOLD_GOODS_NO_LEGAL_OR_THEFT,
    InsuranceProductTypePrimo.VACATION_HOME_HOUSEHOLD_GOODS,
  ].includes(t as any);
  const isTopBiz = (t: string) => t === InsuranceProductTypeTopBiz.BUSINESS_MOVABLE_PROPERTY;
  const isGw = (lobName: string | null | undefined) => lobName === LobName.PRIVATE_CONTENT;

  const isPrivateContentInsurance = (p: any): boolean => {
    switch (p.system) {
      case InsuranceSystem.PRIMO:    return isPrimo(p.insuranceProductType);
      case InsuranceSystem.TOP_BIZ:  return isTopBiz(p.insuranceProductType);
      case InsuranceSystem.GUIDEWIRE: return isGw(p.guidewireIdentifier?.lobName);
      case InsuranceSystem.UNKNOWN:  return false;
    }
    return false;
  };

  const isProduct = useCallback((p: any): boolean => {
    switch (p.system) {
      case InsuranceSystem.PRIMO:    return isPrimo(p.insuranceProductType);
      case InsuranceSystem.TOP_BIZ:  return false; // temporary in real code
      case InsuranceSystem.GUIDEWIRE: return isGw(p.guidewireIdentifier?.lobName);
      case InsuranceSystem.UNKNOWN:  return false;
    }
    return false;
  }, []);

  const privateContentInsurances = useMemo(
    () => (insuranceProducts ?? []).filter(isProduct),
    [insuranceProducts, isProduct],
  );

  return {
    hasPrivateContentInsurance: privateContentInsurances.length > 0,
    privateContentInsurances,
    isPrivateContentInsurance,
  };
};
