// Pattern: Expo runtime globals
// Demonstrates: when the project depends on `expo`, hermes-test runs the project's own
// `expo/src/winter/runtime.native.ts` before tests — exactly what `expo/src/Expo.fx` does at app
// start. So Expo apps see Expo's globals (TextDecoder, WHATWG URL, structuredClone, spec FormData)
// on top of React Native's, using the installed Expo version. `fetch` stays hermes-test's mock
// (Expo's own EXPO_PUBLIC_USE_RN_FETCH opt-out). Disable with "expoRuntime": false in config.
import { test, group, expect } from 'hermes-test';

group('Expo runtime globals (expo/src/winter)', () => {
  test('TextDecoder exists for Expo apps (it does not for bare React Native)', () => {
    expect(typeof TextDecoder).toBe('function');
    const bytes = new TextEncoder().encode('héllo');
    expect(new TextDecoder().decode(bytes)).toBe('héllo');
  });

  test('URL is WHATWG (whatwg-url-minimum) — non-http schemes have a host, unlike bare RN', () => {
    const u = new URL('s3://bucket/key?X-Amz-Signature=secret');
    expect(u.protocol).toBe('s3:');
    expect(u.host).toBe('bucket');
    expect(u.pathname).toBe('/key');
    expect(u.searchParams.get('X-Amz-Signature')).toBe('secret');
    expect(new URL('../b', 'https://x.test/a/c/d').href).toBe('https://x.test/a/b');
  });

  test('structuredClone is available', () => {
    const src = { a: [1, { b: new Date(0) }] };
    const copy = structuredClone(src);
    expect(copy).toEqual(src);
    expect(copy).not.toBe(src);
  });

  test('FormData has the spec methods Expo patches in (get/has/entries/delete)', () => {
    const fd = new FormData();
    fd.append('a', '1');
    fd.append('a', '2');
    expect(fd.get('a')).toBe('1');
    expect(fd.getAll('a')).toEqual(['1', '2']);
    expect(fd.has('a')).toBe(true);
    expect(Array.from(fd.entries())).toEqual([['a', '1'], ['a', '2']]);
    fd.delete('a');
    expect(fd.has('a')).toBe(false);
  });

  test('fetch is still hermes-test\'s mock, not expo/fetch (native)', async () => {
    const { http, HttpResponse } = await import('hermes-test');
    ht.mock.fetch(http.get('https://example.com/activate-expo', () => HttpResponse.json({})));
    const res = await fetch('https://example.com/unmocked-expo');
    expect(res.status).toBe(500);
    expect(await res.text()).toContain('[mock.fetch] Unhandled GET');
  });
});
