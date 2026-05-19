// Real production code — faithful copy
// Changes: inline getInsuranceParentProductTypePrimo + LINKS, no monorepo imports

enum InsuranceProductTypePrimo {
  INDBO = 'INDBO', ANNUAL_TRAVEL = 'ANNUAL_TRAVEL', PERSONAL_CAR = 'PERSONAL_CAR',
  CAT = 'CAT', DOG = 'DOG', HORSE = 'HORSE', HOUSEHOLD_GOODS = 'HOUSEHOLD_GOODS',
}
enum InsuranceParentProductTypePrimo { TRAVEL = 'TRAVEL', CAR = 'CAR', HOME = 'HOME', OTHER = 'OTHER' }
const LINKS = { COVERAGE_EXTEND_TRAVEL: '/travel/extend/', COVERAGE_EXTEND_CAR: '/car/extend/' };

function getInsuranceParentProductTypePrimo(type: string): InsuranceParentProductTypePrimo {
  if (type === InsuranceProductTypePrimo.ANNUAL_TRAVEL) return InsuranceParentProductTypePrimo.TRAVEL;
  if (type === InsuranceProductTypePrimo.PERSONAL_CAR) return InsuranceParentProductTypePrimo.CAR;
  return InsuranceParentProductTypePrimo.OTHER;
}

const primoCoverageHelper = ({ coverageData, coverageItems, insuranceMetaInfo, coverageId, deductibleText, mitTopdanmarkDomain }: any) => {
  const coverages = insuranceMetaInfo?.coverages ?? [];
  const primoCoverage = coverages.find((c: any) => c.coverageId === coverageId);
  const cmsCoverage = coverageData?.find((c: any) => c.coverageItemId === coverageId);
  const title = primoCoverage?.name ?? '';
  const subtitle = cmsCoverage?.subtitle ?? '';

  const getDeductibles = () => {
    const coverageObjects = coverages.filter((c: any) => c.coverageId === coverageId && c.deductibles);
    const primaryCoverage = coverageObjects.shift();
    const baseDeductible: any = primaryCoverage?.deductibles?.[0]?.textForDisplay?.trim() !== ''
      ? { ...(primaryCoverage?.deductibles?.[0] ?? {}) }
      : {};

    if (Object.keys(baseDeductible).length === 0 && coverageObjects.length <= 1) return undefined;

    let secondaryTextForDisplay: string | undefined;
    if (['CAT', 'DOG', 'HORSE'].includes(insuranceMetaInfo.productType)) {
      const tiered = insuranceMetaInfo.formattedBestFeltValues?.find((it: any) => it.bestFelt === 'EGEN-BETAL-SYGDAEK');
      if (tiered) secondaryTextForDisplay = tiered.promptText + ' ' + tiered.value;
    }

    const exceptions = coverageObjects.map((c: any) => {
      const d = c.deductibles?.[0];
      return {
        textForDisplay: d?.deductPercent ? deductibleText : (d?.textForDisplay ?? undefined),
        deductAmount: d?.deductAmount ?? undefined,
        title: c.name,
      };
    });

    const notes = coverageObjects
      .filter((c: any) => c.deductibles?.[0]?.deductPercent)
      .map((c: any) => c.deductibles ? `${c.name}: ${c.deductibles[0]?.textForDisplay}` : '');

    return {
      textForDisplay: baseDeductible.textForDisplay,
      secondaryTextForDisplay,
      deductAmount: baseDeductible.deductAmount,
      exceptions,
      notes,
    };
  };

  const getCoverageDetails = () => {
    if (!cmsCoverage) return { included: { title: '', items: [] }, excluded: { title: '', items: [] } };
    const { included, excluded } = cmsCoverage;
    return {
      included: { title: included?.title ?? '', items: included?.items?.map((i: string) => ({ text: i, icon: 'checkmark', direction: 'right' })) ?? [] },
      excluded: { title: excluded?.title ?? '', items: excluded?.items?.map((i: string) => ({ text: i, icon: 'xmark', direction: 'right' })) ?? [] },
    };
  };

  const getExtendCoverageLink = () => {
    const parent = getInsuranceParentProductTypePrimo(insuranceMetaInfo.productType);
    let link = '';
    if (parent === InsuranceParentProductTypePrimo.TRAVEL) link = LINKS.COVERAGE_EXTEND_TRAVEL;
    if (parent === InsuranceParentProductTypePrimo.CAR) link = LINKS.COVERAGE_EXTEND_CAR;
    return link === '' ? null : mitTopdanmarkDomain + link + insuranceMetaInfo.insuranceAgreementIdRaw;
  };

  const getFutureInEffectData = () => {
    const item = coverageItems?.find((c: any) => c.id === coverageId);
    if (!item || !item.isSelected || !item.futureInEffectDate) return null;
    return { futureInEffectDate: item.futureInEffectDate, futureInEffectPrice: item.coveragePrice ?? '' };
  };

  return { title, subtitle, deductibles: getDeductibles(), details: getCoverageDetails(), extendCoverageLink: getExtendCoverageLink(), futureInEffectData: getFutureInEffectData() };
};

export default primoCoverageHelper;
