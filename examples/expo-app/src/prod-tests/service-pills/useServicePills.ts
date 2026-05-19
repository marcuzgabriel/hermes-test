// Real hook — generates service pill items from insurance coverages/products
import { useMemo } from 'react';
import { useSelector } from 'react-redux';

const PILL_COVERAGE_IDS: Record<string, string> = {
  CARGLASS: 'carglass', CARWASH: 'carwash', WHEEL_CHANGE: 'wheelChange', ROADSIDE_ASSISTANCE: 'roadsideAssistance',
};

export const pillsFromCoverages = (coverages: any[], t: (s: string) => string) => {
  const pills: any[] = [];
  for (const cov of coverages) {
    const id = cov.coverageId?.toLowerCase() ?? '';
    if (id.includes('carglass') || id.includes('glas')) pills.push({ title: t('carglassTitle'), type: 'carglass', coverageId: cov.coverageId });
    if (id.includes('carwash') || id.includes('vask')) pills.push({ title: t('carwashTitle'), type: 'carwash', coverageId: cov.coverageId });
    if (id.includes('wheel') || id.includes('hjul')) pills.push({ title: t('wheelChangeTitle'), type: 'wheelChange', coverageId: cov.coverageId });
    if (id.includes('roadside') || id.includes('vejhjælp')) pills.push({ title: t('roadsideTitle'), type: 'roadsideAssistance', coverageId: cov.coverageId });
  }
  return pills;
};

export const pillsFromInsurances = (insuranceProducts: any[], selectedIndex: number, t: (s: string) => string) => {
  const pills: any[] = [];
  const seen = new Set<string>();
  for (const product of insuranceProducts) {
    const type = product.insuranceProductType?.toLowerCase() ?? '';
    if (type.includes('car') && !seen.has('carglass')) {
      seen.add('carglass');
      pills.push({ title: t('carglassTitle'), type: 'carglass', productIndex: selectedIndex });
    }
  }
  return pills;
};

export const useServicePills = (selectedIndex: number = 0) => {
  const coverages = useSelector((s: any) => s.insurance?.coverages ?? []);
  const products = useSelector((s: any) => s.insurance?.products ?? []);
  const t = (s: string) => s;

  const fromCoverages = useMemo(() => pillsFromCoverages(coverages, t), [coverages]);
  const fromInsurances = useMemo(() => pillsFromInsurances(products, selectedIndex, t), [products, selectedIndex]);

  return { pills: [...fromCoverages, ...fromInsurances] };
};
