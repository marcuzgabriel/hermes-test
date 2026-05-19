// Real hook — derives TopBiz meta info from insurance details in store
import { useMemo } from 'react';
import { useSelector } from 'react-redux';

export const useGetInsuranceMetaInfoTopBiz = (selectedProduct: any) => {
  const insuranceDetails = useSelector((s: any) => s.insuranceDetails?.topBiz ?? null);

  return useMemo(() => {
    if (!insuranceDetails) return null;
    const stakeholders = insuranceDetails.stakeholders ?? [];
    const coverages = insuranceDetails.coverages ?? [];
    const productType = selectedProduct?.insuranceProductType ?? 'UNKNOWN';
    const activities = insuranceDetails.insuranceObjectList?.[0]?.activityList ?? [];
    const operatingLosses = insuranceDetails.insuranceObjectList?.[0]?.turnOver?.operatingLosses;

    return {
      productType,
      stakeholders,
      coverages,
      activities,
      operatingLosses,
      insuranceObjectList: insuranceDetails.insuranceObjectList,
      title: insuranceDetails.title ?? '',
      effectiveDate: insuranceDetails.effectiveDate,
    };
  }, [insuranceDetails, selectedProduct]);
};
