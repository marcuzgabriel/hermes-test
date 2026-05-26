// Pattern: Redux selector hooks with withStore
// Demonstrates: withStore, patchState, renderHookWithReduxStore
import { test, group, beforeEach, withStore, expect } from 'hermes-test';
import { useSelector } from 'react-redux';
import { useMemo } from 'react';

// --- Hook under test ---
function useCartSummary() {
  const items: any[] = useSelector((s: any) => s.cart.items);
  const filter: string = useSelector((s: any) => s.cart.filter);

  return useMemo(() => {
    const filtered = filter ? items.filter((i) => i.category === filter) : items;
    const total = filtered.reduce((sum, i) => sum + i.price * i.qty, 0);
    return { items: filtered, count: filtered.length, total };
  }, [items, filter]);
}

// --- Tests ---
let ctx: ReturnType<typeof withStore>;
const item = (name: string, price: number, qty: number, category: string) =>
  ({ name, price, qty, category });

beforeEach(() => {
  ctx = withStore({ cart: { items: [], filter: '' } });
});

group('useCartSummary', () => {
  test('empty cart returns zero', () => {
    const { current } = ctx.renderHookWithReduxStore(() => useCartSummary());
    expect(current.count).toBe(0);
    expect(current.total).toBe(0);
    expect(current.items).toEqual([]);
  });

  test('sums items correctly', () => {
    ctx.patchState({ cart: {
      items: [item('Apple', 2, 3, 'fruit'), item('Bread', 4, 1, 'bakery')],
      filter: '',
    }});
    const { current } = ctx.renderHookWithReduxStore(() => useCartSummary());
    expect(current.count).toBe(2);
    expect(current.total).toBe(10); // 2*3 + 4*1
  });

  test('filters by category', () => {
    ctx.patchState({ cart: {
      items: [item('Apple', 2, 3, 'fruit'), item('Bread', 4, 1, 'bakery')],
      filter: 'fruit',
    }});
    const { current } = ctx.renderHookWithReduxStore(() => useCartSummary());
    expect(current.count).toBe(1);
    expect(current.total).toBe(6);
    expect(current.items[0].name).toBe('Apple');
  });

  test('patchState updates reflected in hook', () => {
    ctx.patchState({ cart: { items: [item('Milk', 3, 2, 'dairy')], filter: '' } });
    const { current } = ctx.renderHookWithReduxStore(() => useCartSummary());
    expect(current.total).toBe(6);
    expect(current.count).toBe(1);
  });
});
