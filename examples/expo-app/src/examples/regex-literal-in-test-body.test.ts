// Pattern: regex literals containing quote characters inside a test file
// Demonstrates: the mock-hoisting pass understands regex literals. A `"` or `'` inside
// `/[...]/` used to open a phantom string in the bundle scanner and — depending on what
// followed in the bundle — either mis-hoisted the next test file or crashed the runner
// (`patches.rs:76` slice panic). This file must sit in the same bundle as other tests.
import { test, group, expect, spy } from 'hermes-test';

ht.mock('./asset-imports/icon-font', () => ({ iconFont: undefined, iconFontName: () => 'Mocked' }));
import { iconFontName } from './asset-imports/icon-font';

group('regex literals in test bodies', () => {
  test('a regex with a double quote does not derail the scanner', () => {
    const unsafe = /[<>:"\/\\|?*]/;
    expect(unsafe.test('a:b')).toBe(true);
    expect(unsafe.test('safe-name')).toBe(false);
  });

  test("a regex with a single quote and a division on the same line", () => {
    const apostrophe = /don't/;
    const half = 10 / 2;
    expect(apostrophe.test("I don't")).toBe(true);
    expect(half).toBe(5);
  });

  test('a regex right after return / typeof keywords', () => {
    const pick = (s: string) => {
      if (typeof /x/ === 'object') return /y/.test(s);
      return false;
    };
    expect(pick('y')).toBe(true);
  });

  test('the mock registered in this file still applies (hoisting intact)', () => {
    const fn = spy(() => iconFontName());
    expect(fn()).toBe('Mocked');
  });
});
