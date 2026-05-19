// Real production hook — faithful copy
// Changes: flattened imports, inline formatPrice, mockModule deps
import { useCallback, useMemo } from 'react';
import { useGetInsuranceMetaInfo } from './useGetInsuranceMetaInfo';

const formatPrice = (n: number) => n.toLocaleString('da-DK');
const TopBizType = {
  LIABILITY: 'LIABILITY', TRANSPORT: 'TRANSPORT', MACHINE: 'MACHINE',
  BUSINESS_INTERRUPTION: 'BUSINESS_INTERRUPTION', OCCUPATIONAL_VOLUNTARY: 'OCCUPATIONAL_VOLUNTARY',
  OCCUPATIONAL_STATUTORY: 'OCCUPATIONAL_STATUTORY', UNKNOWN: 'UNKNOWN',
};

export const useGetCoverageInfoTopBiz = ({ coverage, selectedInsuranceProduct }: { coverage: any; selectedInsuranceProduct: any }) => {
  const t = (s: string) => s; // identity i18n
  const insuranceMetaInfo = useGetInsuranceMetaInfo(selectedInsuranceProduct);
  const coverageTitle = coverage.coverageType.name;
  const coverageCategory = coverage.coverageType.category?.text;
  const coverageSubtitle = coverageTitle !== coverageCategory && coverageCategory ? coverageCategory : '';

  const { insuranceProductType, mainStakeholderNameAndAddress, activities, coveragePeriod } = useMemo(() => {
    let stakeholder = '', productType = TopBizType.UNKNOWN, acts = null, period = null;
    const topBizStakeholders = insuranceMetaInfo.stakeholders ?? [];
    const { insuranceObjectList, productType: pt } = insuranceMetaInfo;
    if (insuranceMetaInfo) {
      productType = pt ?? TopBizType.UNKNOWN;
      const company = topBizStakeholders[0]?.company;
      if (company) {
        stakeholder = company.name;
        if (company.address?.domesticAddress) {
          const { streetName, zipCode } = company.address.domesticAddress;
          stakeholder = [stakeholder, streetName, zipCode].filter(Boolean).join(', ');
        }
      }
      if (insuranceObjectList?.[0]?.activityList) acts = insuranceObjectList[0].activityList;
      if (insuranceObjectList?.[0]?.turnOver?.operatingLosses) period = insuranceObjectList[0].turnOver.operatingLosses;
    }
    return { insuranceProductType: productType, mainStakeholderNameAndAddress: stakeholder, activities: acts, coveragePeriod: period };
  }, [insuranceMetaInfo]);

  const meta: any = { coverageInfoTitleBundles: [], deductibleTitle: '', deductibleAmount: '', deductibleDescription: '', coverageTitle, coverageSubtitle };

  const addDefaultFields = useCallback(() => {
    const items: any[] = [{ disabled: true, title: t('coverageAmount'), subtitle: coverage.coverageType.coverageAmount1 ? formatPrice(coverage.coverageType.coverageAmount1) : '' }];
    if (coverage.coverageDetailsList?.[0]?.type === 'regularInsurance') {
      const d = coverage.coverageDetailsList[0];
      items.push({ disabled: true, title: d.description.replace('- ', ''), subtitle: d.amount || d.amount === 0 ? formatPrice(d.amount) : t('yes') });
    }
    meta.coverageInfoTitleBundles.push({ items });
  }, [coverage, meta, t]);

  if (coverage.deductibles.length > 0 && coverage.deductibles[0]?.deductAmount) {
    meta.deductibleTitle = formatPrice(coverage.deductibles[0].deductAmount);
    meta.deductibleAmount = `${coverage.deductibles[0].deductAmount}`;
  }

  switch (insuranceProductType) {
    case TopBizType.LIABILITY: {
      const isVariable = coverage.deductibles[0]?.deductText.includes('%');
      if (isVariable) { meta.deductibleTitle = t('deductibleVariableShort'); meta.deductibleDescription = coverage.deductibles[0]?.extendedTerms ?? ''; }
      else if (coverage.deductibles[0]?.deductAmount) meta.deductibleTitle = formatPrice(coverage.deductibles[0].deductAmount);
      const fallback = coverage.coverageType.category?.id === 'MA' ? t('perTheLaw') : '';
      meta.coverageInfoTitleBundles.push({ items: [{ disabled: true, title: t('coverageAmountMaxPerYear'), subtitle: coverage.coverageType.coverageAmount1 ? formatPrice(coverage.coverageType.coverageAmount1) : fallback }] });
      break;
    }
    case TopBizType.TRANSPORT: {
      meta.deductibleDescription = t('coverageAmountPerActivity');
      const items: any[] = [];
      activities?.forEach((a: any) => { if (a.typeOfBusiness?.text) items.push({ disabled: true, title: a.typeOfBusiness.text, subtitle: formatPrice(a.transportActivityAmount ?? 0) }); });
      meta.coverageInfoTitleBundles.push({ items });
      break;
    }
    case TopBizType.MACHINE: {
      if (coverage.coverageDetailsList) {
        coverage.coverageDetailsList.filter((d: any) => d.type === 'machineInsurance').forEach((d: any) => {
          meta.coverageInfoTitleBundles.push({ items: [
            { disabled: true, title: t('machineType'), subtitle: d.machineType },
            { disabled: true, title: t('identification'), subtitle: d.identification },
            { disabled: true, title: t('machineCoverage'), subtitle: coverage.coverageType.name },
            { disabled: true, title: t('sum'), subtitle: formatPrice(d.sum) },
          ], headerLabel: `${t('machineSingle')} ${d.machineId}` });
        });
      } else addDefaultFields();
      break;
    }
    case TopBizType.BUSINESS_INTERRUPTION: {
      addDefaultFields();
      if (coveragePeriod) meta.coverageInfoTitleBundles.push({ items: [{ disabled: true, title: t('coveragePeriod'), subtitle: `${coveragePeriod} ${t('months')}` }] });
      break;
    }
    case TopBizType.OCCUPATIONAL_VOLUNTARY: case TopBizType.OCCUPATIONAL_STATUTORY: {
      if (coverage.coverageType.coverageAmount1 && coverage.coverageType.coverageAmount2) {
        meta.coverageInfoTitleBundles.push({ items: [
          { disabled: true, title: t('coverageAmountMaxPerEmployeePerAccident'), subtitle: formatPrice(coverage.coverageType.coverageAmount1) },
          { disabled: true, title: t('coverageAmountMaxPerAccident'), subtitle: formatPrice(coverage.coverageType.coverageAmount2) },
        ]});
      }
      break;
    }
    default: addDefaultFields();
  }

  if (coverage.coInsuredStakeholderList.length > 0) {
    const names = [mainStakeholderNameAndAddress];
    coverage.coInsuredStakeholderList.forEach((s: any) => {
      if (s.company) {
        let n = s.company.name;
        if (s.company.address?.domesticAddress) {
          const { streetName, zipCode } = s.company.address.domesticAddress;
          n = [n, streetName, zipCode].filter(Boolean).join(', ');
        }
        names.push(n);
      }
    });
    meta.coverageInfoTitleBundles.push({ items: [{ disabled: true, title: t('aboutInsuranceMetaSecured'), subtitle: names.join('\n') }] });
  }

  return meta;
};
