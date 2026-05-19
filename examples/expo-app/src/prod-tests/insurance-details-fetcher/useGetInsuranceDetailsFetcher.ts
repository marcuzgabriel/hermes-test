// Real production hook — faithful copy
// Changes: uses mockModule deps, useSelector for store, spy-able dispatch functions
import { useCallback, useMemo, useState } from 'react';
import { useSelector } from 'react-redux';

export enum FetchingStatus { FETCHING = 'FETCHING', DID_FETCH = 'DID_FETCH' }
export enum InsuranceSystem { PRIMO = 'PRIMO', TOP_BIZ = 'TOP_BIZ', GUIDEWIRE = 'GUIDEWIRE', UNKNOWN = 'UNKNOWN' }
const InsuranceProductTypeTopBiz = { MACHINE: 'MACHINE' };

// These are passed in from tests via the config object
interface FetcherConfig {
  dispatchPrimo: (action: any) => Promise<any>;
  dispatchTopBiz: (action: any) => Promise<any>;
  dispatchGuidewire: (action: any) => Promise<any>;
  dispatchAgreementPeriod: (action: any) => Promise<any>;
  primoInitiate: (args: any, opts?: any) => any;
  topBizInitiate: (args: any, opts?: any) => any;
  topBizMachineInitiate: (args: any, opts?: any) => any;
  guidewireInitiate: (args: any, opts?: any) => any;
  agreementPeriodInitiate: (args: any, opts?: any) => any;
}

export const useGetInsuranceDetailsFetcher = (hasDetails: boolean, config: FetcherConfig) => {
  const [fetchingStatus, setFetchingStatus] = useState<FetchingStatus>(
    hasDetails ? FetchingStatus.DID_FETCH : FetchingStatus.FETCHING,
  );

  const insurances = useSelector((s: any) => s.insurances?.data ?? []);

  const fetchInsuranceDetails = useCallback(
    async (insurance: any, props?: { forceRefetch: boolean }) => {
      try {
        setFetchingStatus(FetchingStatus.FETCHING);

        const product = insurances
          ?.flatMap((i: any) => i.products)
          ?.find((p: any) => p.insuranceNumbers?.at(0) === insurance.insuranceProductId);

        switch (insurance.system) {
          case InsuranceSystem.PRIMO:
            if (product?.detailsFetchable === false) return;
            break;
          case InsuranceSystem.TOP_BIZ:
            if (product?.detailsFetchable === false) return;
            break;
          case InsuranceSystem.GUIDEWIRE:
            if (insurance?.guidewireIdentifier?.shouldFetchDetails === false) return;
            break;
        }

        const forceRefetch = props?.forceRefetch ? { forceRefetch: true } : undefined;

        switch (insurance.system) {
          case InsuranceSystem.PRIMO:
            await config.dispatchPrimo(
              config.primoInitiate({ insuranceAgreementNumber: insurance.insuranceAgreementNumber }, forceRefetch)
            );
            break;

          case InsuranceSystem.TOP_BIZ: {
            const endpoint = insurance.insuranceProductType === InsuranceProductTypeTopBiz.MACHINE
              ? config.topBizMachineInitiate
              : config.topBizInitiate;

            const detailsPromise = config.dispatchTopBiz(
              endpoint({ insuranceAgreementNumber: insurance.insuranceAgreementNumber, insuranceProductId: insurance.insuranceProductId ?? 0 }, forceRefetch)
            );
            const agreementPromise = config.dispatchAgreementPeriod(
              config.agreementPeriodInitiate({ insuranceAgreementNumber: insurance.insuranceAgreementNumber }, forceRefetch)
            );
            await Promise.allSettled([detailsPromise, agreementPromise]);
            break;
          }

          case InsuranceSystem.GUIDEWIRE:
            if (insurance.guidewireIdentifier?.lobName) {
              const payload: any = {
                insuranceAgreementNumber: insurance.insuranceAgreementNumber,
                lobName: insurance.guidewireIdentifier.lobName,
              };
              const filterCoverable = insurance.guidewireIdentifier.lobFlavorVirtualName ?? insurance.guidewireIdentifier.lobFlavorCoverableId;
              if (filterCoverable) payload.filterCoverable = filterCoverable;
              await config.dispatchGuidewire(config.guidewireInitiate(payload, forceRefetch));
            }
            break;
        }
      } finally {
        setFetchingStatus(FetchingStatus.DID_FETCH);
      }
    },
    [insurances, config],
  );

  return useMemo(() => ({
    fetchingStatus,
    fetchInsuranceDetails,
  }), [fetchingStatus, fetchInsuranceDetails]);
};
