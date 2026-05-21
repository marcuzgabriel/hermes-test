// Production test port: primoCoverageHelper — 7 tests
// Original: apps/topdanmark/src/areas/Insurance/Coverage/helpers/__tests__/primoCoverageHelper.test.ts
//
// Pure function. Coverage data aggregation for Primo insurance UI.

const { test, group, expect } = (globalThis as any).__HT;
import primoCoverageHelper from './primoCoverageHelper';

const coverageData = [{
  coverageItemId: 'coverage1',
  included: { title: 'Included Title', items: ['Item1'] },
  excluded: { title: 'Excluded Title', items: ['Item2'] },
  subtitle: 'Coverage Subtitle',
  title: 'Coverage Title',
  nonCustomerDescription: 'Non Customer Description',
  customerDescription: 'Customer Description',
  termsId: ['123'],
}];

const insuranceMetaInfo = {
  insuranceAgreementId: '123',
  insuranceAgreementIdRaw: '123',
  productType: 'INDBO',
  title: 'Title',
  coverages: [{
    coverageId: 'coverage1', name: 'Coverage Name', originalEffectiveDate: '2020-01-01',
    chosen: true, deductibles: [{ textForDisplay: 'Deductible Text', deductAmount: '100' }],
  }],
};

const baseArgs = {
  coverageData,
  coverageItems: [],
  insuranceMetaInfo,
  coverageId: 'coverage1',
  deductibleText: 'Custom Deductible Text',
  mitTopdanmarkDomain: 'https://mit.topdanmark.dk',
};

group('primoCoverageHelper', () => {
  test('returns expected object with valid input', ({ expect }: any) => {
    const result = primoCoverageHelper(baseArgs);
    expect(result.title).toBe('Coverage Name');
    expect(result.subtitle).toBe('Coverage Subtitle');
    expect(result.deductibles.textForDisplay).toBe('Deductible Text');
    expect(result.deductibles.deductAmount).toBe('100');
    expect(result.deductibles.exceptions).toEqual([]);
    expect(result.deductibles.notes).toEqual([]);
    expect(result.details.included.title).toBe('Included Title');
    expect(result.details.included.items.length).toBe(1);
    expect(result.details.excluded.title).toBe('Excluded Title');
    expect(result.extendCoverageLink).toBeNull();
    expect(result.futureInEffectData).toBeNull();
  });

  test('handles missing deductible information', ({ expect }: any) => {
    const result = primoCoverageHelper({
      ...baseArgs,
      insuranceMetaInfo: { ...insuranceMetaInfo, coverages: [{ coverageId: 'coverage1', name: 'Coverage Name', chosen: true, originalEffectiveDate: '2020-01-01' }] },
    });
    expect(result.deductibles).toBeUndefined();
  });

  test('handles deductible percent with exceptions and notes', ({ expect }: any) => {
    const result = primoCoverageHelper({
      ...baseArgs,
      insuranceMetaInfo: {
        ...insuranceMetaInfo,
        coverages: [
          { coverageId: 'coverage1', name: 'Coverage Name', chosen: true, originalEffectiveDate: '2020-01-01',
            deductibles: [{ textForDisplay: 'Deductible Text', deductAmount: '100', deductPercent: '10' }] },
          { coverageId: 'coverage1', name: 'Coverage Name', chosen: true, originalEffectiveDate: '2020-01-01',
            deductibles: [{ textForDisplay: 'Exception', deductAmount: '100', deductPercent: '10' }] },
        ],
      },
    });
    expect(result.deductibles.textForDisplay).toBe('Deductible Text');
    expect(result.deductibles.exceptions.length).toBe(1);
    expect(result.deductibles.exceptions[0].textForDisplay).toBe('Custom Deductible Text');
    expect(result.deductibles.notes).toEqual(['Coverage Name: Exception']);
  });

  test('handles invalid coverageId gracefully', ({ expect }: any) => {
    const result = primoCoverageHelper({ ...baseArgs, coverageId: 'invalidCoverageId' });
    expect(result.title).toBe('');
    expect(result.subtitle).toBe('');
    expect(result.deductibles).toBeUndefined();
    expect(result.details.included.items).toEqual([]);
    expect(result.details.excluded.items).toEqual([]);
  });

  test('generates travel extend link', ({ expect }: any) => {
    const result = primoCoverageHelper({
      ...baseArgs,
      insuranceMetaInfo: { ...insuranceMetaInfo, productType: 'ANNUAL_TRAVEL' },
    });
    expect(result.extendCoverageLink).toBe('https://mit.topdanmark.dk/travel/extend/123');
  });

  test('generates car extend link', ({ expect }: any) => {
    const result = primoCoverageHelper({
      ...baseArgs,
      insuranceMetaInfo: { ...insuranceMetaInfo, productType: 'PERSONAL_CAR' },
    });
    expect(result.extendCoverageLink).toBe('https://mit.topdanmark.dk/car/extend/123');
  });

  test('returns futureInEffectData when coverage item has future date', ({ expect }: any) => {
    const result = primoCoverageHelper({
      ...baseArgs,
      coverageItems: [{ id: 'coverage1', isSelected: true, futureInEffectDate: '2025-01-01', coveragePrice: '500' }],
    });
    expect(result.futureInEffectData).toEqual({ futureInEffectDate: '2025-01-01', futureInEffectPrice: '500' });
  });
});
