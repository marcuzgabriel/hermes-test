// Real production hook — faithful copy
// Changes: flattened imports, useSelector for store access, mocked deps via local imports
import { useMemo } from 'react';
import { useSelector } from 'react-redux';
import { getInsuranceTitles } from './getInsuranceTitles';

const InsuranceSystem = { PRIMO: 'PRIMO', TOP_BIZ: 'TOP_BIZ', GUIDEWIRE: 'GUIDEWIRE' };
const InsuranceProductTypePrimo = { WAGE_INSURANCE: 'WAGE_INSURANCE' };

export const useGetInsurancesOverview = (insuranceProducts: any[] | null) => {
  // In prod: useI18n().t — identity function in tests
  const t = (s: string) => s;

  const overview = useMemo(() => {
    if (!insuranceProducts) return null;
    const items: any[] = [];

    insuranceProducts.forEach((product: any) => {
      const selectedInsuranceProduct = {
        system: product.system,
        insuranceAgreementNumber: product.insuranceAgreementNumber,
        insuranceProductType: product.insuranceProductType,
        guidewireIdentifier: product.guidewireIdentifier ?? null,
        insuranceProductId: product.insuranceNumbers?.at(0),
        insuranceDetailsId: product.insuranceDetails?.id ?? null,
      };

      const titles = getInsuranceTitles(product);
      if (!titles) return;

      let showExternalLink = false;
      if (product.system === InsuranceSystem.PRIMO && product.insuranceProductType === InsuranceProductTypePrimo.WAGE_INSURANCE) {
        showExternalLink = true;
      }

      items.push({
        selectedInsuranceProduct,
        title: t(titles.title),
        subtitle: titles.subtitle,
        showExternalLink,
      });
    });
    return items;
  }, [insuranceProducts]);

  return overview;
};

export const findSelectedInsuranceProductIndex = (
  selected: any | null | undefined,
  overviews: any[],
) => {
  if (!selected) return -1;
  return overviews.findIndex((o: any) => {
    const p = o.selectedInsuranceProduct;
    return p.system === selected.system &&
      p.insuranceAgreementNumber === selected.insuranceAgreementNumber &&
      p.insuranceProductType === selected.insuranceProductType &&
      p.guidewireIdentifier === selected.guidewireIdentifier &&
      p.insuranceProductId === selected.insuranceProductId;
  });
};

export const useGetIndexBySelectedInsuranceProduct = (selected: any | null | undefined) => {
  const insuranceProducts = useSelector((s: any) => s.insurances?.products ?? null);
  const overview = useGetInsurancesOverview(insuranceProducts) as any[];
  return findSelectedInsuranceProductIndex(selected, overview);
};

export const useGetSelectedInsuranceProductByIndex = (index: number) => {
  const insuranceProducts = useSelector((s: any) => s.insurances?.products ?? null);
  const overview = useGetInsurancesOverview(insuranceProducts);
  return overview?.[index]?.selectedInsuranceProduct;
};
