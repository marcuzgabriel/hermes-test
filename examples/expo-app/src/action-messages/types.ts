// Faithful reproduction of the real app's types (without Zod dependency)

export type DocumentMessageType =
  | 'CAR_MILLAGE_ABOVE'
  | 'CAR_MILLAGE_BELOW'
  | 'CAR_MILLAGE_MISSING'
  | 'CLAIM_TDC'
  | 'CLAIM_MITMESSAGE'
  | 'CLAIM_MITMESSAGE_MULTIPLE'
  | 'CLAIM_INVOICE'
  | 'POA_PENDING_CPR'
  | 'POA_PENDING_CVR'
  | 'POA_CREATE_CPR'
  | 'POA_CREATE_CVR'
  | 'MISSING_INFO_ODOMETER_READING'
  | 'MISSING_INFO_CHIP_ID'
  | 'MISSING_MC_TOURING_NUMBER'
  | 'MPS_ACTIVATION_GW';

export interface ActionMessage {
  messageType: string;
  optionalData: {
    document: {
      id: string;
      content: string;
      message_type: DocumentMessageType;
      icon: string;
      priority: string;
      title: string;
      tracking_name: string;
      name: string;
      calltoactionbutton: {
        buttonText: string;
        opennewwindow: boolean;
        externallink: string;
      };
    };
    // Union fields from different data shapes
    insuranceNumber?: string;
    policyNumber?: string;
    regNo?: string;
    vehicleType?: string;
    claimNumber?: string;
    claimLossType?: string;
    claimMessage?: string;
    uploadLinkEnabled?: boolean;
    messageCount?: number;
    messageType?: string;
  };
}

export type ClaimData = ActionMessage['optionalData'] & { claimNumber?: string; claimLossType?: string };
export type OdometerData = ActionMessage['optionalData'] & { policyNumber: string };
export type AdditionalInfoData = ActionMessage['optionalData'] & { insuranceNumber: string };

export enum FormType {
  MILEAGE = 'mileage',
  CARGLASS = 'carglass',
  UNKNOWN = 'unknown',
}

export const ENDPOINTS_MESSAGES = {
  GET_ACTION_MESSAGES: {
    method: 'GET' as const,
    name: 'getActionMessages',
    url: '/assistant-service-api-v2/v1/messages',
  },
  DISMISS_ACTION_MESSAGE: {
    method: 'POST' as const,
    name: 'dismissActionMessage',
    url: '/assistant-service-api-v2/v1/messages/dismiss',
  },
};
