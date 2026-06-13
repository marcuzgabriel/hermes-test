// Pattern: Advanced fetch mocking — all HTTP methods, body assertions, overrides
// Demonstrates: mockFetch, mockFetchUse, mockFetchReset, http, HttpResponse
import { test, group, afterEach, http, HttpResponse, flushAsync, expect, mock } from 'hermes-test';

// --- Base handlers ---
mock.fetch(
  http.get('https://api.example.com/items', () =>
    HttpResponse.json([{ id: 1, name: 'Widget' }])),
  http.post('https://api.example.com/items', (req: any) =>
    HttpResponse.json({ id: 2, ...req.body }, { status: 201 })),
  http.put('https://api.example.com/items/1', (req: any) =>
    HttpResponse.json({ id: 1, ...req.body })),
  http.delete('https://api.example.com/items/1', () =>
    HttpResponse.json(null, { status: 204 })),
);

afterEach(() => { mock.fetch.reset(); });

// --- Tests ---
group('GET requests', () => {
  test('fetches item list', () => {
    const res = flushAsync(fetch('https://api.example.com/items').then(r => r.json()));
    expect(res).toEqual([{ id: 1, name: 'Widget' }]);
  });

  test('per-test override returns different data', () => {
    mock.fetch.overwrite(http.get('https://api.example.com/items', () =>
      HttpResponse.json([{ id: 99, name: 'Override' }])));
    const res = flushAsync(fetch('https://api.example.com/items').then(r => r.json()));
    expect(res[0].name).toBe('Override');
  });
});

group('POST requests', () => {
  test('creates an item', () => {
    const res = flushAsync(
      fetch('https://api.example.com/items', {
        method: 'POST', body: JSON.stringify({ name: 'Gadget' }),
        headers: { 'Content-Type': 'application/json' },
      }).then(r => r.json()));
    expect(res.name).toBe('Gadget');
    expect(res.id).toBe(2);
  });
});

group('PUT requests', () => {
  test('updates an item', () => {
    const res = flushAsync(
      fetch('https://api.example.com/items/1', {
        method: 'PUT', body: JSON.stringify({ name: 'Updated' }),
        headers: { 'Content-Type': 'application/json' },
      }).then(r => r.json()));
    expect(res.name).toBe('Updated');
    expect(res.id).toBe(1);
  });
});

group('DELETE requests', () => {
  test('deletes an item', () => {
    const res = flushAsync(fetch('https://api.example.com/items/1', { method: 'DELETE' }));
    expect(res.status).toBe(204);
  });
});

group('error responses', () => {
  test('server error override', () => {
    mock.fetch.overwrite(http.get('https://api.example.com/items', () =>
      HttpResponse.json({ error: 'down' }, { status: 500 })));
    const res = flushAsync(fetch('https://api.example.com/items'));
    expect(res.status).toBe(500);
  });
});
