// Real hook — derives Guidewire meta info from insurance details in store
import { useMemo } from 'react';
import { useSelector } from 'react-redux';

export const useGetInsuranceMetaInfoGuidewire = (selectedProduct: any) => {
  const insuranceDetails = useSelector((s: any) => s.insuranceDetails?.guidewire ?? null);

  return useMemo(() => {
    if (!insuranceDetails) return null;
    const coverages = insuranceDetails.coverages ?? [];
    const terms = insuranceDetails.terms ?? [];
    const lobName = selectedProduct?.guidewireIdentifier?.lobName ?? '';

    return {
      lobName,
      coverages,
      terms,
      title: insuranceDetails.title ?? '',
      insuredContentList: insuranceDetails.insuredContentList,
      productType: lobName,
    };
  }, [insuranceDetails, selectedProduct]);
};
