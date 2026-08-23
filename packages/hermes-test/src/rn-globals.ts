// React Native runtime globals sourced from the packages RN itself uses
// (react-native/Libraries/Core/setUpXHR.js): `whatwg-fetch` for Headers/Request/Response and
// `abort-controller` for AbortController/AbortSignal. Bundled into the harness — users install
// nothing. Installed only when absent so an app-level polyfill run earlier wins.
//
// `fetch` itself is NOT taken from whatwg-fetch (it needs XMLHttpRequest); hermes-test's
// handler-based mock fetch owns globalThis.fetch (src/fetch.ts), and polyfills.js pre-installs
// a stub so whatwg-fetch does not install its XHR-based one.
import { Headers, Request, Response } from 'whatwg-fetch';
import { AbortController, AbortSignal } from 'abort-controller';

export function installReactNativeGlobals(g: any = globalThis): void {
  if (typeof g.Headers === 'undefined') g.Headers = Headers;
  if (typeof g.Request === 'undefined') g.Request = Request;
  if (typeof g.Response === 'undefined') g.Response = Response;
  if (typeof g.AbortController === 'undefined') g.AbortController = AbortController;
  if (typeof g.AbortSignal === 'undefined') g.AbortSignal = AbortSignal;
}
