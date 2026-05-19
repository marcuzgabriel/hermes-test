// Real production code — faithful copy of nestCoverages.ts
// Only change: removed createLogger dependency (no-op in tests)

export enum DeductibleStateType {
  DEDUCTIBLE_VARIABLE = 'deductibleVariable',
  DEDUCTIBLE_SPECIFIC = 'deductibleSpecific',
  DEDUCTIBLE_NONE = 'deductibleNone',
}

export interface CoveragePrimo {
  name: string;
  chosen: boolean;
  coverageId: string;
  originalEffectiveDate: string | null;
  deductibles?: Array<{ code?: string; textForDisplay: string; deductAmount: string }>;
}

export interface DeductibleState { label: string; type: DeductibleStateType; }

export interface CoverageGrp {
  name: string;
  deductible: DeductibleState;
  coverageId: string;
  list: CoveragePrimo[];
  chosen: boolean;
}

const nestCoverages = (coverages: CoveragePrimo[]): CoverageGrp[] => {
  try {
    const list: CoverageGrp[] = [];
    const deductMap: Map<string, Set<string>> = new Map();

    coverages.forEach(coverage => {
      const result = list.find(item => item.coverageId === coverage.coverageId);
      countDeductible(deductMap, coverage);

      if (result) {
        result.deductible = getDeductibleState(deductMap, coverage.coverageId);
        result.list.push(coverage);
      } else {
        list.push({
          coverageId: coverage.coverageId,
          name: coverage.name,
          deductible: getDeductibleState(deductMap, coverage.coverageId),
          chosen: coverage.chosen,
          list: [coverage],
        });
      }
    });
    return list;
  } catch (e) {
    return [];
  }
};

export const countDeductible = (deductMap: Map<string, Set<string>>, coverage: CoveragePrimo) => {
  if (!deductMap.has(coverage.coverageId)) deductMap.set(coverage.coverageId, new Set());
  const tmp = deductMap.get(coverage.coverageId);
  if (tmp && coverage.deductibles?.[0]) tmp.add(coverage.deductibles[0].textForDisplay);
};

export const getDeductibleState = (deductMap: Map<string, Set<string>>, coverageId: string): DeductibleState => {
  const tmp = deductMap.get(coverageId);
  if (tmp && tmp.size > 1) return { type: DeductibleStateType.DEDUCTIBLE_VARIABLE, label: DeductibleStateType.DEDUCTIBLE_VARIABLE };
  const val = tmp?.values().next().value;
  if (!val || val === '0 kr') return { type: DeductibleStateType.DEDUCTIBLE_NONE, label: DeductibleStateType.DEDUCTIBLE_NONE };
  return { label: val, type: DeductibleStateType.DEDUCTIBLE_SPECIFIC };
};

export default nestCoverages;
