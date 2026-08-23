// Pattern: React Native runtime globals
// Demonstrates: the harness provides what React Native installs in InitializeCore
// (Libraries/Core/setUpXHR.js) — FormData, URL, URLSearchParams — with the same shape RN
// gives you on device. It deliberately does NOT add things RN lacks (e.g. TextDecoder), so
// code that would throw on a phone throws here too.
import { test, group, expect } from 'hermes-test';

group('React Native globals', () => {
  group('FormData (Libraries/Network/FormData.js shape)', () => {
    test('append / getAll', () => {
      const fd = new FormData();
      fd.append('a', '1');
      fd.append('a', '2');
      fd.append('b', 'x');
      expect(fd.getAll('a')).toEqual(['1', '2']);
      expect(fd.getAll('missing')).toEqual([]);
    });

    test('getParts mirrors RN for strings and file-like values', () => {
      const fd = new FormData();
      fd.append('title', 'hello');
      fd.append('photo', { uri: 'file:///p.jpg', name: 'p.jpg', type: 'image/jpeg' } as any);
      const parts = (fd as any).getParts();
      expect(parts[0]).toEqual({
        string: 'hello',
        fieldName: 'title',
        headers: { 'content-disposition': 'form-data; name="title"' },
      });
      expect(parts[1].fieldName).toBe('photo');
      expect(parts[1].uri).toBe('file:///p.jpg');
      expect(parts[1].headers['content-type']).toBe('image/jpeg');
      expect(parts[1].headers['content-disposition']).toBe('form-data; name="photo"; filename="p.jpg"');
    });
  });

  group('URL (Libraries/Blob/URL.js shape)', () => {
    test('strips userinfo from host like RN', () => {
      const u = new URL('https://user:secret@files.example.com:8443/path?x=1#frag');
      expect(u.host).toBe('files.example.com:8443');
      expect(u.hostname).toBe('files.example.com');
      expect(u.port).toBe('8443');
      expect(u.username).toBe('user');
      expect(u.password).toBe('secret');
      expect(u.pathname).toBe('/path');
      expect(u.search).toBe('?x=1');
      expect(u.hash).toBe('#frag');
      expect(u.origin).toBe('https://files.example.com:8443');
      expect(u.searchParams.get('x')).toBe('1');
    });

    test('non-http schemes expose protocol but no host — same as RN on device', () => {
      const u = new URL('s3://bucket/key?X-Amz-Signature=secret');
      expect(u.protocol).toBe('s3:');
      expect(u.host).toBe('');
      expect(String(u)).toBe('s3://bucket/key?X-Amz-Signature=secret');
    });

    test('does not invent globals React Native lacks', () => {
      // RN 0.8x installs TextEncoder via Hermes but not TextDecoder; keep parity so code
      // relying on TextDecoder fails in tests the same way it fails on a phone.
      expect(typeof (globalThis as any).TextDecoder).toBe('undefined');
    });
  });
});
