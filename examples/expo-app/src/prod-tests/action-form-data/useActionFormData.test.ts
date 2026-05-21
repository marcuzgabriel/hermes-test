import { test, group, beforeEach, expect } from 'hermes-test';
// Production test port: useActionFormData — 13 tests
// Original: apps/topdanmark/src/areas/Claims/ActionForm/hooks/__tests__/useActionFormData.test.ts
//
// Hook reads RTK cache from store. Tests seed store.api.queries with action data + config.
// Complex: datepicker ISO conversion, list merging, radio mapping, checkbox zones.

import { withStore } from '../shared/testStore';
import { useActionFormData } from './useActionFormData';

let ctx: ReturnType<typeof withStore>;

// Seed the store to mimic RTK Query cache
function mockRTKCache(action: any, config: any) {
  ctx.patchState({
    api: {
      queries: {
        'getActionGroup({"actionGroupId":"group-1"})': { data: { customerActions: [action] } },
        'getActionTypeConfig(undefined)': { data: { actionConfigs: [config] } },
      },
    },
  });
}

// --- Minimal mock action data (extracted from real mocks) ---
const whenAndWhereAction = {
  actionId: 'a-when-where',
  data: {
    section: 'UHELDET_HVORNAAR_HVOR',
    fields: [
      { component: 'datepicker', name: 'uheldetHvornaarHvorDate', label: 'Dato', value: '15/02-2024' },
      { component: 'timepicker', name: 'uheldetHvornaarHvorTime', label: 'Tid', value: '01:00' },
      { component: 'text', name: 'uheldetHvornaarHvorText', label: 'Sted', value: 'Østerbrogade 24, København' },
    ],
  },
};

const whenAndWhereConfig = {
  actionType: 'UHELDET_HVORNAAR_HVOR',
  formType: 'DATEPICKER_TIME_FREE_TEXT_INPUT',
  title: 'Hvornår og hvor',
  version: '1.0',
  fields: [
    { component: 'datepicker', name: 'uheldetHvornaarHvorDate', label: 'Dato' },
    { component: 'timepicker', name: 'uheldetHvornaarHvorTime', label: 'Tid' },
    { component: 'text', name: 'uheldetHvornaarHvorText', label: 'Sted' },
  ],
};

const whatHappenedAction = {
  actionId: 'a-what-happened',
  data: {
    section: 'UHELDET_HAENDELSEN',
    fields: [
      { component: 'radio', name: 'uheldetHaendelsenRadio', label: 'Hændelse', value: 'Parkeringsskade' },
      { component: 'textarea', name: 'uheldetHaendelsenElaborate', label: 'Uddyb', value: 'Bilen blev ridset langs siden på parkeringspladsen' },
    ],
  },
};

const whatHappenedConfig = {
  actionType: 'UHELDET_HAENDELSEN',
  formType: 'SELECTOR_INCIDENT_AUTO',
  title: 'Hændelse',
  version: '1.0',
  fields: [
    { component: 'radio', name: 'uheldetHaendelsenRadio', label: 'Hændelse', options: [{ label: 'Parkeringsskade', valueID: { selected: 'Parkeringsskade' } }] },
    { component: 'textarea', name: 'uheldetHaendelsenElaborate', label: 'Uddyb' },
  ],
};

const faultAssessmentAction = {
  actionId: 'a-fault',
  data: { section: 'UHELDET_SKYLD_OPFATTELSE', fields: [
    { component: 'radio', name: 'uheldetSkyldOpfattelseRadio', label: 'Skyld', value: 'En anden person' },
  ]},
};

const faultAssessmentConfig = {
  actionType: 'UHELDET_SKYLD_OPFATTELSE', formType: 'SELECTOR_NO_CONTENT', title: 'Skyld', version: '1.0',
  fields: [{ component: 'radio', name: 'uheldetSkyldOpfattelseRadio', label: 'Skyld' }],
};

const vehicleDamageAction = {
  actionId: 'a-damage',
  data: { section: 'SKADE_KOERETOEJ', fields: [
    { component: 'checkbox', name: 'skadeKoeretoejFront', label: 'Front', value: 'Front' },
    { component: 'checkbox', name: 'skadeKoeretoejOtherWindshield', label: 'Forrude', value: 'Forrude' },
    { component: 'checkbox', name: 'skadeKoeretoejOtherRoof', label: 'Tag', value: 'Tag' },
  ]},
};

const vehicleDamageConfig = {
  actionType: 'SKADE_KOERETOEJ', formType: 'DAMAGE_ZONES', title: 'Skader', version: '1.0',
  fields: [
    { component: 'checkbox', name: 'skadeKoeretoejFront', label: 'Front' },
    { component: 'checkbox', name: 'skadeKoeretoejOtherWindshield', label: 'Forrude' },
    { component: 'checkbox', name: 'skadeKoeretoejOtherRoof', label: 'Tag' },
  ],
};

const driverIsInsideHouseholdAction = {
  actionId: 'a-driver',
  data: { section: 'BILENS_FOERER', fields: [
    { component: 'radio', name: 'bilensFoererRadio', label: 'Fører', value: 'En anden fra min husstand' },
    { component: 'text', name: 'bilensFoererInsideHouseholdName', label: 'Navn', value: 'Test' },
    { component: 'text', name: 'bilensFoererInsideHouseholdAge', label: 'Alder', value: '32' },
    { component: 'text', name: 'bilensFoererInsideHouseholdPhone', label: 'Telefon', value: '43214321' },
  ]},
};

const driverConfig = {
  actionType: 'BILENS_FOERER', formType: 'SELECTOR_DRIVER', title: 'Fører', version: '1.0',
  fields: [
    { component: 'radio', name: 'bilensFoererRadio', label: 'Fører' },
    { component: 'text', name: 'bilensFoererInsideHouseholdName', label: 'Navn' },
    { component: 'text', name: 'bilensFoererInsideHouseholdAge', label: 'Alder' },
    { component: 'text', name: 'bilensFoererInsideHouseholdPhone', label: 'Telefon' },
  ],
};

const counterPartyTwoAction = {
  actionId: 'a-counter',
  data: { section: 'OPLYSNINGER_OM_MODPARTER_2', fields: [
    { component: 'list', name: 'oplysningerOmModparter2CounterPartyListItem', label: 'Første modpart',
      valueID: { items: [{ name: 'oplysningerOmModparter2NumberPlate', value: '11111' }] } },
    { component: 'list', name: 'oplysningerOmModparter2CounterPartyListItem', label: 'Anden modpart',
      valueID: { items: [{ name: 'oplysningerOmModparter2NumberPlate', value: '222' }] } },
  ]},
};

const counterPartyConfig = {
  actionType: 'OPLYSNINGER_OM_MODPARTER_2', formType: 'SELECTOR_DYNAMIC_LIST', title: 'Modparter', version: '1.0',
  fields: [{ component: 'list', name: 'oplysningerOmModparter2CounterPartyList', label: 'Modpart' }],
};

const independentWitnessesAction = {
  actionId: 'a-witness',
  data: { section: 'UVILDIGE_VIDNER_TIL_UHELDET', fields: [
    { component: 'radio', name: 'uvildigeVidnerTilUheldetRadio', label: 'Vidner', value: 'Ja' },
    { component: 'list', name: 'uvildigeVidnerTilUheldetWitnessListItem', label: 'Første vidne',
      valueID: { items: [
        { name: 'uvildigeVidnerTilUheldetPhone', value: '43243212' },
        { name: 'uvildigeVidnerTilUheldetName', value: 'Test' },
        { name: 'uvildigeVidnerTilUheldetEmail', value: 'test@if.dk' },
      ]}},
    { component: 'list', name: 'uvildigeVidnerTilUheldetWitnessListItem', label: 'Andet vidne',
      valueID: { items: [
        { name: 'uvildigeVidnerTilUheldetPhone', value: '43213232232' },
        { name: 'uvildigeVidnerTilUheldetName', value: 'Test23232' },
        { name: 'uvildigeVidnerTilUheldetEmail', value: 'test23333@if.dk' },
      ]}},
  ]},
};

const reportedToPoliceAction = {
  actionId: 'a-police',
  data: { section: 'MELDT_TIL_POLITI_2', fields: [
    { component: 'radio', name: 'meldtTilPoliti2Radio', label: 'Politi', value: 'Nej' },
  ]},
};

const reportedToPoliceConfig = {
  actionType: 'MELDT_TIL_POLITI_2', formType: 'SELECTOR_NO_CONTENT', title: 'Politi', version: '1.0',
  fields: [{ component: 'radio', name: 'meldtTilPoliti2Radio', label: 'Politi',
    options: [{ label: 'Ja', valueID: { selected: 'Ja' } }, { label: 'Nej', valueID: { selected: 'Nej' } }] }],
};

// =============================================
beforeEach(() => {
  ctx = withStore({ api: { queries: {} } });
});

group('useActionFormData', () => {
  test('returns null when cache is empty', () => {
    const { current } = ctx.renderHookWithReduxStore(() =>
      useActionFormData('group-1', 'action-1', 'UHELDET_HAENDELSEN', 'SELECTOR_INCIDENT_AUTO'));
    expect(current.actionConfig).toBeNull();
    expect(current.actionData).toBeNull();
  });

  group('step 1 — when/where (datepicker)', () => {
    test('transforms DD/MM-YYYY to ISO format', () => {
      mockRTKCache(whenAndWhereAction, whenAndWhereConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-when-where', 'UHELDET_HVORNAAR_HVOR', 'DATEPICKER_TIME_FREE_TEXT_INPUT'));
      const dateField = current.actionData?.fields.find((f: any) => f.name === 'uheldetHvornaarHvorDate');
      expect(dateField?.value).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    });

    test('preserves time field value as-is', () => {
      mockRTKCache(whenAndWhereAction, whenAndWhereConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-when-where', 'UHELDET_HVORNAAR_HVOR', 'DATEPICKER_TIME_FREE_TEXT_INPUT'));
      const timeField = current.actionData?.fields.find((f: any) => f.name === 'uheldetHvornaarHvorTime');
      expect(timeField?.value).toBe('01:00');
    });

    test('preserves address text field', () => {
      mockRTKCache(whenAndWhereAction, whenAndWhereConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-when-where', 'UHELDET_HVORNAAR_HVOR', 'DATEPICKER_TIME_FREE_TEXT_INPUT'));
      const locationField = current.actionData?.fields.find((f: any) => f.name === 'uheldetHvornaarHvorText');
      expect(locationField?.value).toBe('Østerbrogade 24, København');
    });
  });

  group('step 2 — what happened (radio + textarea)', () => {
    test('maps radio and textarea correctly', () => {
      mockRTKCache(whatHappenedAction, whatHappenedConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-what-happened', 'UHELDET_HAENDELSEN', 'SELECTOR_INCIDENT_AUTO'));
      expect(current.actionData?.fields.find((f: any) => f.name === 'uheldetHaendelsenRadio')?.value).toBe('Parkeringsskade');
      expect(current.actionData?.fields.find((f: any) => f.name === 'uheldetHaendelsenElaborate')?.value).toBe('Bilen blev ridset langs siden på parkeringspladsen');
    });
  });

  group('step 3 — fault assessment', () => {
    test('maps radio selection correctly', () => {
      mockRTKCache(faultAssessmentAction, faultAssessmentConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-fault', 'UHELDET_SKYLD_OPFATTELSE', 'SELECTOR_NO_CONTENT'));
      expect(current.actionData?.fields.find((f: any) => f.name === 'uheldetSkyldOpfattelseRadio')?.value).toBe('En anden person');
    });
  });

  group('step 4 — vehicle damage zones', () => {
    test('maps checkbox fields correctly', () => {
      mockRTKCache(vehicleDamageAction, vehicleDamageConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-damage', 'SKADE_KOERETOEJ', 'DAMAGE_ZONES'));
      const fields = current.actionData?.fields ?? [];
      expect(fields.find((f: any) => f.name === 'skadeKoeretoejFront')?.value).toBe('Front');
      expect(fields.find((f: any) => f.name === 'skadeKoeretoejOtherWindshield')?.value).toBe('Forrude');
      expect(fields.find((f: any) => f.name === 'skadeKoeretoejOtherRoof')?.value).toBe('Tag');
    });
  });

  group('step 5 — driver info', () => {
    test('maps radio + conditional fields', () => {
      mockRTKCache(driverIsInsideHouseholdAction, driverConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-driver', 'BILENS_FOERER', 'SELECTOR_DRIVER'));
      const fields = current.actionData?.fields ?? [];
      expect(fields.find((f: any) => f.name === 'bilensFoererRadio')?.value).toBe('En anden fra min husstand');
      expect(fields.find((f: any) => f.name === 'bilensFoererInsideHouseholdName')?.value).toBe('Test');
      expect(fields.find((f: any) => f.name === 'bilensFoererInsideHouseholdAge')?.value).toBe('32');
      expect(fields.find((f: any) => f.name === 'bilensFoererInsideHouseholdPhone')?.value).toBe('43214321');
    });
  });

  group('step 6 — counter parties (dynamic list)', () => {
    test('merges two list items into a single JSON array', () => {
      mockRTKCache(counterPartyTwoAction, counterPartyConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-counter', 'OPLYSNINGER_OM_MODPARTER_2', 'SELECTOR_DYNAMIC_LIST'));
      const listField = current.actionData?.fields.find((f: any) => f.name === 'oplysningerOmModparter2CounterPartyList');
      expect(listField).toBeDefined();
      const parsed = JSON.parse(listField?.value ?? '[]');
      expect(parsed.length).toBe(2);
      expect(parsed[0].ordinal).toBe('Første modpart');
      expect(parsed[0].fields.oplysningerOmModparter2NumberPlate).toBe('11111');
      expect(parsed[1].ordinal).toBe('Anden modpart');
      expect(parsed[1].fields.oplysningerOmModparter2NumberPlate).toBe('222');
    });

    test('strips Item suffix from list field name', () => {
      mockRTKCache(counterPartyTwoAction, counterPartyConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-counter', 'OPLYSNINGER_OM_MODPARTER_2', 'SELECTOR_DYNAMIC_LIST'));
      expect(current.actionData?.fields.find((f: any) => f.name === 'oplysningerOmModparter2CounterPartyListItem')).toBeUndefined();
      expect(current.actionData?.fields.find((f: any) => f.name === 'oplysningerOmModparter2CounterPartyList')).toBeDefined();
    });
  });

  group('step 7 — witnesses (dynamic list)', () => {
    test('preserves radio selection', () => {
      mockRTKCache(independentWitnessesAction, {});
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-witness', 'UVILDIGE_VIDNER_TIL_UHELDET', 'SELECTOR_DYNAMIC_LIST'));
      expect(current.actionData?.fields.find((f: any) => f.name === 'uvildigeVidnerTilUheldetRadio')?.value).toBe('Ja');
    });

    test('merges two witnesses into JSON array', () => {
      mockRTKCache(independentWitnessesAction, {});
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-witness', 'UVILDIGE_VIDNER_TIL_UHELDET', 'SELECTOR_DYNAMIC_LIST'));
      const listField = current.actionData?.fields.find((f: any) => f.name === 'uvildigeVidnerTilUheldetWitnessList');
      const parsed = JSON.parse(listField?.value ?? '[]');
      expect(parsed.length).toBe(2);
      expect(parsed[0].ordinal).toBe('Første vidne');
      expect(parsed[0].fields.uvildigeVidnerTilUheldetPhone).toBe('43243212');
      expect(parsed[0].fields.uvildigeVidnerTilUheldetName).toBe('Test');
      expect(parsed[0].fields.uvildigeVidnerTilUheldetEmail).toBe('test@if.dk');
      expect(parsed[1].ordinal).toBe('Andet vidne');
      expect(parsed[1].fields.uvildigeVidnerTilUheldetPhone).toBe('43213232232');
    });
  });

  group('step 9 — reported to police', () => {
    test('maps radio with ValueID', () => {
      mockRTKCache(reportedToPoliceAction, reportedToPoliceConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-police', 'MELDT_TIL_POLITI_2', 'SELECTOR_NO_CONTENT'));
      expect(current.actionData?.fields.find((f: any) => f.name === 'meldtTilPoliti2Radio')?.value).toBe('Nej');
    });

    test('maps config fields and options', () => {
      mockRTKCache(reportedToPoliceAction, reportedToPoliceConfig);
      const { current } = ctx.renderHookWithReduxStore(() =>
        useActionFormData('group-1', 'a-police', 'MELDT_TIL_POLITI_2', 'SELECTOR_NO_CONTENT'));
      expect(current.actionConfig?.formType).toBe('SELECTOR_NO_CONTENT');
      expect(current.actionConfig?.title).toBe('Politi');
      const radioConfig = current.actionConfig?.fields.find((f: any) => f.name === 'meldtTilPoliti2Radio');
      expect(radioConfig?.options?.length).toBe(2);
    });
  });
});
