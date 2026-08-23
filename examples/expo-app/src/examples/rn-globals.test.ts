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

  group('Headers / Request / Response come from whatwg-fetch (what RN ships)', () => {
    test('Headers is case-insensitive, appends, and iterates in insertion order', () => {
      const h = new Headers({ 'Content-Type': 'text/plain' });
      h.append('x-a', '1');
      h.append('X-A', '2');
      expect(h.get('content-type')).toBe('text/plain');
      expect(h.get('x-a')).toBe('1, 2');
      expect(h.has('X-A')).toBe(true);
      expect(Array.from(h.keys())).toEqual(['content-type', 'x-a']);
      expect(Array.from(h.entries())).toEqual([['content-type', 'text/plain'], ['x-a', '1, 2']]);
      h.delete('x-a');
      expect(h.has('x-a')).toBe(false);
    });

    test('Request normalizes method and carries headers/body like the spec', async () => {
      const req = new Request('https://example.com/api', {
        method: 'post',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ a: 1 }),
      });
      expect(req.method).toBe('POST');
      expect(req.url).toBe('https://example.com/api');
      expect(req.headers.get('content-type')).toBe('application/json');
      expect(await req.text()).toBe('{"a":1}');
      expect(req.bodyUsed).toBe(true);
    });

    test('Response exposes ok/status, json(), text() and clone()', async () => {
      const res = new Response('{"ok":true}', { status: 201, headers: { 'content-type': 'application/json' } });
      expect(res.ok).toBe(true);
      expect(res.status).toBe(201);
      const copy = res.clone();
      expect(await res.json()).toEqual({ ok: true });
      expect(await copy.text()).toBe('{"ok":true}');
      expect(new Response(null, { status: 404 }).ok).toBe(false);
      expect(Response.error().type).toBe('error');
    });

    test('fetch itself stays hermes-test\'s mock (whatwg-fetch\'s XHR fetch is not installed)', async () => {
      const res = await fetch('https://example.com/unmocked');
      expect(res.ok).toBe(false);
      expect(res.status).toBe(500);
      expect(await res.text()).toContain('[mock.fetch] Unhandled GET');
    });

    test('mock fetch reads the body of a spec Request object', async () => {
      const { http, HttpResponse } = await import('hermes-test');
      let seen: any = null;
      ht.mock.fetch(http.post('https://example.com/echo', (req) => { seen = req; return HttpResponse.json({ ok: true }); }));
      const req = new Request('https://example.com/echo', { method: 'POST', body: JSON.stringify({ title: 'New Post' }), headers: { 'content-type': 'application/json' } });
      const res = await fetch(req);
      expect(res.ok).toBe(true);
      expect(seen.method).toBe('POST');
      expect(seen.body).toEqual({ title: 'New Post' });
      expect(seen.headers['content-type']).toBe('application/json');
    });

    test('console has the full React Native surface (assert is what event-target-shim calls)', () => {
      expect(typeof console.assert).toBe('function');
      expect(typeof console.group).toBe('function');
      expect(typeof console.table).toBe('function');
      expect(() => console.assert(true, 'never printed')).not.toThrow();
    });
  });

  group('AbortController / AbortSignal come from abort-controller (what RN ships)', () => {
    test('abort() flips the signal, fires the event and listeners, and is idempotent', () => {
      const ac = new AbortController();
      let fired = 0;
      let viaOnabort = 0;
      ac.signal.addEventListener('abort', () => { fired++; });
      ac.signal.onabort = () => { viaOnabort++; };
      expect(ac.signal.aborted).toBe(false);
      expect(ac.signal instanceof AbortSignal).toBe(true);
      ac.abort();
      ac.abort();
      expect(ac.signal.aborted).toBe(true);
      expect(fired).toBe(1);
      expect(viaOnabort).toBe(1);
    });

    test('removeEventListener detaches a listener', () => {
      const ac = new AbortController();
      let fired = 0;
      const l = () => { fired++; };
      ac.signal.addEventListener('abort', l);
      ac.signal.removeEventListener('abort', l);
      ac.abort();
      expect(fired).toBe(0);
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
