// Real production hook — Guidewire coverage info
import { useMemo } from 'react';
import { useSelector } from 'react-redux';

const formatPrice = (n: number) => `${n.toLocaleString('da-DK')} kr.`;

export const useGetCoverageInfoGuidewire = ({ insuranceDetails, coverageData, lobName, selectedInsuranceProduct }: any) => {
  const coverages = insuranceDetails?.coverages ?? [];
  const terms = insuranceDetails?.terms ?? [];

  const coverageTitle = useMemo(() => {
    const cms = coverageData?.find((c: any) => c.coverageItemId === coverages[0]?.publicID);
    return cms?.title ?? coverages[0]?.name ?? '';
  }, [coverages, coverageData]);

  const coverageSubtitle = useMemo(() => {
    const cms = coverageData?.find((c: any) => c.coverageItemId === coverages[0]?.publicID);
    return cms?.subtitle ?? '';
  }, [coverages, coverageData]);

  const deductibles = useMemo(() => {
    const deductTerms = terms.filter((t: any) => t.modelType_Top === 'Deductible' || /Deduct/i.test(t.type ?? ''));
    if (deductTerms.length === 0) return { textForDisplay: 'deductibleNone', deductAmount: '', exceptions: [], notes: [] };

    const main = deductTerms[0];
    const value = main.chosenTermValue ?? main.directValue ?? '';
    const isMoney = main.valueTypeCode_Top === 'money';
    const textForDisplay = isMoney && value ? formatPrice(Number(value)) : (value || 'deductibleNone');

    const exceptions = deductTerms.slice(1).map((t: any) => ({
      title: t.name ?? '', textForDisplay: t.chosenTermValue ? (t.valueTypeCode_Top === 'money' ? formatPrice(Number(t.chosenTermValue)) : t.chosenTermValue) : '',
    }));

    return { textForDisplay: value ? textForDisplay : 'deductibleNone', deductAmount: value ? String(value) : '', exceptions, notes: [] };
  }, [terms]);

  const coverageInsuredSums = useMemo(() => {
    return terms.filter((t: any) => t.modelType_Top === 'InsuredSum' || /Sum.*Insured/i.test(t.type ?? ''))
      .map((t: any) => ({ title: t.name ?? 'Forsikringssum', content: t.chosenTermValue ? formatPrice(Number(t.chosenTermValue)) : (t.directStringValue ?? '') }));
  }, [terms]);

  const coverageDetails = useMemo(() => {
    const cms = coverageData?.find((c: any) => c.coverageItemId === coverages[0]?.publicID);
    if (!cms) return { included: { title: '', items: [] }, excluded: { title: '', items: [] } };
    return {
      included: { title: cms.included?.title ?? 'Dækker', items: (cms.included?.items ?? []).map((i: string) => ({ text: i, icon: 'checkmark' })) },
      excluded: { title: cms.excluded?.title ?? 'Dækker ikke', items: (cms.excluded?.items ?? []).map((i: string) => ({ text: i, icon: 'xmark' })) },
    };
  }, [coverages, coverageData]);

  const extendCoverageLink = useMemo(() => {
    if (lobName === 'travel') return `https://mit.topdanmark.dk/travel/extend/${selectedInsuranceProduct?.insuranceAgreementNumber ?? ''}`;
    return null;
  }, [lobName, selectedInsuranceProduct]);

  return { coverageTitle, coverageSubtitle, deductibles, coverageInsuredSums, coverageDetails, extendCoverageLink };
};
