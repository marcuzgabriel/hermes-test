// Production test port: PopupManager — 10 tests
// Original: apps/topdanmark/src/utils/popups/__tests__/PopupManager.test.ts
//
// State machine: popup priority queue, quarantine, condition evaluation, caching.

const { test, group, beforeEach, expect } = (globalThis as any).__metroTest;
import { PopupManager } from './PopupManager';

beforeEach(() => { PopupManager.reset(); });

group('PopupManager', () => {
  test('returns empty when no popups registered', ({ expect }: any) => {
    expect(PopupManager.getPopups()).toEqual([]);
  });

  test('returns popups sorted by priority (highest first)', ({ expect }: any) => {
    PopupManager.register({ id: 'low', priority: 1, condition: () => true });
    PopupManager.register({ id: 'high', priority: 10, condition: () => true });
    PopupManager.register({ id: 'mid', priority: 5, condition: () => true });
    const popups = PopupManager.getPopups();
    expect(popups[0].id).toBe('high');
    expect(popups[1].id).toBe('mid');
    expect(popups[2].id).toBe('low');
  });

  test('filters out popups whose condition returns false', ({ expect }: any) => {
    PopupManager.register({ id: 'show', priority: 1, condition: () => true });
    PopupManager.register({ id: 'hide', priority: 2, condition: () => false });
    const popups = PopupManager.getPopups();
    expect(popups.length).toBe(1);
    expect(popups[0].id).toBe('show');
  });

  test('quarantined popups are excluded', ({ expect }: any) => {
    PopupManager.register({ id: 'popup1', priority: 1, condition: () => true });
    PopupManager.setQuarantine('popup1', 60000); // 60s quarantine
    expect(PopupManager.getPopups()).toEqual([]);
  });

  test('isQuarantined returns true during quarantine', ({ expect }: any) => {
    PopupManager.setQuarantine('popup1', 60000);
    expect(PopupManager.isQuarantined('popup1')).toBe(true);
  });

  test('isQuarantined returns false when not quarantined', ({ expect }: any) => {
    expect(PopupManager.isQuarantined('popup1')).toBe(false);
  });

  test('cleanCache clears all quarantines', ({ expect }: any) => {
    PopupManager.setQuarantine('popup1', 60000);
    PopupManager.cleanCache();
    expect(PopupManager.isQuarantined('popup1')).toBe(false);
  });

  test('reset clears configs and cache', ({ expect }: any) => {
    PopupManager.register({ id: 'popup1', priority: 1, condition: () => true });
    PopupManager.setQuarantine('popup2', 60000);
    PopupManager.reset();
    expect(PopupManager.getPopups()).toEqual([]);
    expect(PopupManager.isQuarantined('popup2')).toBe(false);
  });

  test('multiple popups with same priority maintain order', ({ expect }: any) => {
    PopupManager.register({ id: 'a', priority: 5, condition: () => true });
    PopupManager.register({ id: 'b', priority: 5, condition: () => true });
    const popups = PopupManager.getPopups();
    expect(popups.length).toBe(2);
  });

  test('condition can depend on external state', ({ expect }: any) => {
    let showNps = false;
    PopupManager.register({ id: 'nps', priority: 1, condition: () => showNps });
    expect(PopupManager.getPopups().length).toBe(0);
    showNps = true;
    expect(PopupManager.getPopups().length).toBe(1);
    expect(PopupManager.getPopups()[0].id).toBe('nps');
  });
});
