// Faithful port of the real useActionMessages hook
// Only change: flattened imports to local files (no monorepo aliases)

import { useMemo } from 'react';
import {
  DocumentMessageType,
  ClaimData,
  OdometerData,
  ActionMessage,
  AdditionalInfoData,
  FormType,
  ENDPOINTS_MESSAGES,
} from './types';
import { useAppSelector } from './useRedux';
import { useIsLoading } from './useIsLoading';
import { useErrorHandling } from './useErrorHandling';

const TYPES_WITH_DYNAMIC_VARIABLES = [
  'POA_PENDING_CPR',
  'POA_PENDING_CVR',
  'POA_CREATE_CPR',
  'POA_CREATE_CVR',
  'MISSING_INFO_ODOMETER_READING',
  'MISSING_INFO_CHIP_ID',
  'MISSING_MC_TOURING_NUMBER',
];

const formTypeMapping: Record<string, FormType> = {
  CAR_MILLAGE_BELOW: FormType.MILEAGE,
  CAR_MILLAGE_ABOVE: FormType.MILEAGE,
  CAR_MILLAGE_MISSING: FormType.MILEAGE,
};

export interface ActionMessageItem {
  key: string;
  title: string;
  subtitle: string | undefined;
  policyNumber?: string;
  claimNumber?: string | undefined;
  docType: DocumentMessageType;
  ctaButton: {
    text: string;
    openNewWindow: boolean;
    link: string;
  };
  formType: FormType | undefined;
  rawMessageObject: ActionMessage;
}

const EXCLUDE_MESSAGE_TYPES = [
  'CLAIM_MITMESSAGE',
  'CLAIM_MIT_MESSAGE',
  'CLAIM_MIT_MESSAGE_MULTIPLE',
  'MISSING_INFO_CHIP_ID',
  'CLAIM_MITMESSAGE_MULTIPLE',
];

// Simplified selector — in real app this reads from RTK Query cache
const actionMessagesSelector = (state: any) => state?.actionMessages;

export const useActionMessages = () => {
  const actionMessages: ActionMessage[] | undefined = useAppSelector(actionMessagesSelector);
  const { isLoading } = useIsLoading(ENDPOINTS_MESSAGES.GET_ACTION_MESSAGES.name);

  const { error, dispatchWithErrorHandler, removeError } = useErrorHandling(
    ENDPOINTS_MESSAGES.GET_ACTION_MESSAGES.name,
  );

  const fetchActionMessages = async (params?: { forceRefetch: boolean }) => {
    await dispatchWithErrorHandler(() => {
      // In real app: actionMessagesApi.endpoints.getActionMessages.initiate(...)
    });
  };

  const dismiss = async (aMsg: ActionMessage) => {
    await dispatchWithErrorHandler(() => {
      // In real app: actionMessagesApi.endpoints.dismissActionMessage.initiate(...)
    });
  };

  const actionMessageItems = useMemo(() => {
    if (!actionMessages) return [];

    return actionMessages.reduce<ActionMessageItem[]>((acc, message, index) => {
      if (EXCLUDE_MESSAGE_TYPES.includes(message.messageType)) return acc;

      const { optionalData } = message;
      const { document } = optionalData || {};
      const { title, content, message_type: docType } = document;

      const policyNumber =
        (optionalData as AdditionalInfoData)?.insuranceNumber ??
        (optionalData as OdometerData)?.policyNumber ??
        undefined;

      const claimNumber = (optionalData as ClaimData)?.claimNumber;
      const claimSubtitle = claimNumber ? `Skadenummer: ${claimNumber}` : undefined;
      const strippedContent = content.replace(/<[^>]*>/g, '');

      acc.push({
        key: `action-message-${index}`,
        title: TYPES_WITH_DYNAMIC_VARIABLES.includes(docType) ? 'Handling påkrævet' : title,
        subtitle: TYPES_WITH_DYNAMIC_VARIABLES.includes(docType)
          ? undefined
          : claimSubtitle || strippedContent,
        policyNumber,
        claimNumber,
        docType,
        ctaButton: {
          text: document?.calltoactionbutton?.buttonText || '',
          openNewWindow: document?.calltoactionbutton?.opennewwindow || false,
          link: document?.calltoactionbutton?.externallink || '',
        },
        formType: document ? formTypeMapping[document?.message_type] : undefined,
        rawMessageObject: message,
      });
      return acc;
    }, []);
  }, [actionMessages]);

  return {
    isLoading,
    error,
    dismissError: () => removeError(ENDPOINTS_MESSAGES.GET_ACTION_MESSAGES.name),
    fetchActionMessages,
    messageItems: actionMessageItems,
    dismiss,
  };
};
