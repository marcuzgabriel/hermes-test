// Real-world test: useActionMessages — ported from Jest to hermes-test
//
// Original: jest.mock('./useRedux', () => ({ useAppSelector: jest.fn() }))
// hermes-test: mockModule('./useRedux', () => ({ useAppSelector: spy(() => ...) }))
//
// 3 module-level mocks, 24 test cases, groups, beforeEach

const { test, group, beforeEach, renderHook, spy, mockModule, expect } =
  (globalThis as any).__metroTest;

// Spies that persist across tests (like const mockFn = jest.fn())
const mockDispatchWithErrorHandler = spy(async () => {});
const mockRemoveError = spy(() => {});

// Mock return value holders — changed per test
let useAppSelectorReturn: any = undefined;
let useIsLoadingReturn: any = { isLoading: false };
let useErrorHandlingReturn: any = {
  error: null,
  dispatchWithErrorHandler: mockDispatchWithErrorHandler,
  removeError: mockRemoveError,
};

// --- Module-level mocks (replaces jest.mock) ---
mockModule('./useRedux', () => ({
  useAppSelector: (_selector: any) => useAppSelectorReturn,
}));
mockModule('./useIsLoading', () => ({
  useIsLoading: (_tag: string) => useIsLoadingReturn,
}));
mockModule('./useErrorHandling', () => ({
  useErrorHandling: (_tag: string) => useErrorHandlingReturn,
}));

// Import AFTER mocks are registered
import { useActionMessages } from './useActionMessages';
import type { ActionMessage } from './types';
import { FormType } from './types';

// --- Helper (same as original) ---
const createMockActionMessage = (
  messageType: string,
  docType: string,
  optionalData: any = {},
): ActionMessage => ({
  messageType,
  optionalData: {
    ...optionalData,
    document: {
      id: '123',
      content: 'Test content',
      message_type: docType,
      icon: 'icon.png',
      priority: 'high',
      title: 'Test Title',
      tracking_name: 'test_tracking',
      name: 'Test Name',
      calltoactionbutton: {
        buttonText: 'Click me',
        opennewwindow: false,
        externallink: 'https://example.com',
      },
      ...optionalData.document,
    },
  },
});

// --- beforeEach: reset spies + defaults (replaces jest.clearAllMocks) ---
beforeEach(() => {
  mockDispatchWithErrorHandler.reset();
  mockRemoveError.reset();
  useAppSelectorReturn = undefined;
  useIsLoadingReturn = { isLoading: false };
  useErrorHandlingReturn = {
    error: null,
    dispatchWithErrorHandler: mockDispatchWithErrorHandler,
    removeError: mockRemoveError,
  };
});

// =============================================
// basic functionality
// =============================================
group('basic functionality', () => {
  test('should return default values when no messages are present', ({ expect }: any) => {
    useAppSelectorReturn = undefined;

    const { current } = renderHook(() => useActionMessages());

    expect(current.isLoading).toBe(false);
    expect(current.error).toBeNull();
    expect(current.messageItems).toEqual([]);
  });

  test('should return loading state correctly', ({ expect }: any) => {
    useAppSelectorReturn = [];
    useIsLoadingReturn = { isLoading: true };

    const { current } = renderHook(() => useActionMessages());

    expect(current.isLoading).toBe(true);
  });

  test('should return error state correctly', ({ expect }: any) => {
    useAppSelectorReturn = [];
    useErrorHandlingReturn = {
      error: 'Test error',
      dispatchWithErrorHandler: mockDispatchWithErrorHandler,
      removeError: mockRemoveError,
    };

    const { current } = renderHook(() => useActionMessages());

    expect(current.error).toBe('Test error');
  });

  test('should call removeError when dismissError is invoked', ({ expect }: any) => {
    useAppSelectorReturn = [];

    const { current } = renderHook(() => useActionMessages());

    current.dismissError();

    expect(mockRemoveError).wasCalled();
    expect(mockRemoveError).wasCalledWith('getActionMessages');
  });
});

// =============================================
// fetchGetActionMessages
// =============================================
group('fetchGetActionMessages', () => {
  test('should dispatch without forceRefetch by default', ({ expect }: any) => {
    useAppSelectorReturn = [];

    const { current } = renderHook(() => useActionMessages());

    current.fetchActionMessages();

    expect(mockDispatchWithErrorHandler).wasCalledOnce();
  });

  test('should dispatch with forceRefetch when specified', ({ expect }: any) => {
    useAppSelectorReturn = [];

    const { current } = renderHook(() => useActionMessages());

    current.fetchActionMessages({ forceRefetch: true });

    expect(mockDispatchWithErrorHandler).wasCalledOnce();
  });
});

// =============================================
// dismiss
// =============================================
group('dismiss', () => {
  test('should dispatch dismissActionMessage', ({ expect }: any) => {
    useAppSelectorReturn = [];
    const mockMessage = createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE');

    const { current } = renderHook(() => useActionMessages());

    current.dismiss(mockMessage);

    expect(mockDispatchWithErrorHandler).wasCalledOnce();
  });
});

// =============================================
// message filtering and parsing
// =============================================
group('message filtering and parsing', () => {
  test('should exclude CLAIM_MITMESSAGE message types', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('CLAIM_MITMESSAGE', 'CLAIM_MITMESSAGE'),
      createMockActionMessage('OTHER_TYPE', 'CAR_MILLAGE_ABOVE'),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    expect(current.messageItems[0].docType).toBe('CAR_MILLAGE_ABOVE');
  });

  test('should exclude CLAIM_MITMESSAGE_MULTIPLE message types', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('CLAIM_MITMESSAGE_MULTIPLE', 'CLAIM_MITMESSAGE_MULTIPLE'),
      createMockActionMessage('OTHER_TYPE', 'CAR_MILLAGE_BELOW'),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    expect(current.messageItems[0].docType).toBe('CAR_MILLAGE_BELOW');
  });

  test('should parse CAR_MILLAGE_ABOVE message correctly', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE', {
        policyNumber: 'POL123',
      }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.docType).toBe('CAR_MILLAGE_ABOVE');
    expect(item.formType).toBe(FormType.MILEAGE);
    expect(item.policyNumber).toBe('POL123');
  });

  test('should parse CAR_MILLAGE_BELOW message correctly', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_BELOW', {
        policyNumber: 'POL456',
      }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.docType).toBe('CAR_MILLAGE_BELOW');
    expect(item.formType).toBe(FormType.MILEAGE);
  });

  test('should parse CAR_MILLAGE_MISSING message correctly', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_MISSING'),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.docType).toBe('CAR_MILLAGE_MISSING');
    expect(item.formType).toBe(FormType.MILEAGE);
  });

  test('should parse claim messages with claim number in subtitle', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CLAIM_TDC', {
        claimNumber: 'CLM-789',
      }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.claimNumber).toBe('CLM-789');
    expect(item.subtitle).toBe('Skadenummer: CLM-789');
  });

  test('should handle POA_PENDING_CPR with dynamic variables', ({ expect }: any) => {
    useAppSelectorReturn = [createMockActionMessage('TEST_TYPE', 'POA_PENDING_CPR')];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.title).toBe('Handling påkrævet');
    expect(item.subtitle).toBeUndefined();
  });

  test('should handle MISSING_INFO_ODOMETER_READING with dynamic variables', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'MISSING_INFO_ODOMETER_READING'),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.title).toBe('Handling påkrævet');
    expect(item.subtitle).toBeUndefined();
  });

  test('should extract policyNumber from insuranceNumber', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE', {
        insuranceNumber: 'INS-999',
      }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    expect(current.messageItems[0].policyNumber).toBe('INS-999');
  });

  test('should extract policyNumber from OdometerData', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE', {
        policyNumber: 'POL-888',
        regNo: 'ABC123',
        vehicleType: 'car',
      }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    expect(current.messageItems[0].policyNumber).toBe('POL-888');
  });

  test('should parse CTA button correctly', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE', {
        document: {
          calltoactionbutton: {
            buttonText: 'Update Now',
            opennewwindow: true,
            externallink: 'https://test.com',
          },
        },
      }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    const item = current.messageItems[0];
    expect(item.ctaButton.text).toBe('Update Now');
    expect(item.ctaButton.openNewWindow).toBe(true);
    expect(item.ctaButton.link).toBe('https://test.com');
  });

  test('should include raw message object in messageItems', ({ expect }: any) => {
    const mockMessage = createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE');
    useAppSelectorReturn = [mockMessage];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    expect(current.messageItems[0].rawMessageObject).toEqual(mockMessage);
  });
});

// =============================================
// multiple messages
// =============================================
group('multiple messages', () => {
  test('should parse multiple messages correctly', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TYPE1', 'CAR_MILLAGE_ABOVE', { policyNumber: 'POL-1' }),
      createMockActionMessage('TYPE2', 'CLAIM_TDC', { claimNumber: 'CLM-1' }),
      createMockActionMessage('TYPE3', 'CAR_MILLAGE_BELOW', { policyNumber: 'POL-2' }),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(3);
    expect(current.messageItems[0].policyNumber).toBe('POL-1');
    expect(current.messageItems[1].claimNumber).toBe('CLM-1');
    expect(current.messageItems[2].policyNumber).toBe('POL-2');
  });

  test('should filter excluded messages from a mixed list', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TYPE1', 'CAR_MILLAGE_ABOVE'),
      createMockActionMessage('CLAIM_MITMESSAGE', 'CLAIM_MITMESSAGE'),
      createMockActionMessage('TYPE2', 'CLAIM_TDC'),
      createMockActionMessage('CLAIM_MITMESSAGE_MULTIPLE', 'CLAIM_MITMESSAGE_MULTIPLE'),
      createMockActionMessage('TYPE3', 'CAR_MILLAGE_BELOW'),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(3);
    expect(current.messageItems.map((m: any) => m.docType)).toEqual([
      'CAR_MILLAGE_ABOVE',
      'CLAIM_TDC',
      'CAR_MILLAGE_BELOW',
    ]);
  });
});

// =============================================
// edge cases
// =============================================
group('edge cases', () => {
  test('should handle empty messages array', ({ expect }: any) => {
    useAppSelectorReturn = [];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems).toEqual([]);
  });

  test('should handle message with missing optional data fields', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TEST_TYPE', 'CAR_MILLAGE_ABOVE', {}),
    ];

    const { current } = renderHook(() => useActionMessages());

    expect(current.messageItems.length).toBe(1);
    expect(current.messageItems[0].policyNumber).toBeUndefined();
    expect(current.messageItems[0].claimNumber).toBeUndefined();
  });

  test('should assign unique keys to each message item', ({ expect }: any) => {
    useAppSelectorReturn = [
      createMockActionMessage('TYPE1', 'CAR_MILLAGE_ABOVE'),
      createMockActionMessage('TYPE2', 'CAR_MILLAGE_BELOW'),
      createMockActionMessage('TYPE3', 'CLAIM_TDC'),
    ];

    const { current } = renderHook(() => useActionMessages());

    const keys = current.messageItems.map((item: any) => item.key);
    expect(keys).toEqual(['action-message-0', 'action-message-1', 'action-message-2']);
  });
});
