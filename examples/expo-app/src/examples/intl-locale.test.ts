// Pattern: Intl locale formatting
// Demonstrates: toLocaleDateString and toLocaleString respect locale parameter
import { test, group, expect } from 'hermes-test';

group('Intl locale formatting', () => {
  group('Date formatting with da-DK locale', () => {
    test('toLocaleDateString respects da-DK locale', () => {
      const date = new Date(2022, 0, 1); // Jan 1, 2022
      const result = date.toLocaleDateString('da-DK', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
      });
      // Danish format: "1. januar 2022"
      expect(result).toContain('januar');
      expect(result).toContain('2022');
    });

    test('toLocaleDateString with short month', () => {
      const date = new Date(2025, 0, 1); // Jan 1, 2025
      const result = date.toLocaleDateString('da-DK', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      });
      // Danish abbreviated: "1. jan. 2025"
      expect(result).toContain('jan');
      expect(result).toContain('2025');
    });

    test('toLocaleDateString with en-US locale differs from da-DK', () => {
      const date = new Date(2022, 0, 1);
      const da = date.toLocaleDateString('da-DK', { month: 'long', day: 'numeric' });
      const en = date.toLocaleDateString('en-US', { month: 'long', day: 'numeric' });
      // Danish uses "januar", English uses "January"
      expect(da).not.toBe(en);
    });
  });

  group('Number formatting with da-DK locale', () => {
    test('toLocaleString formats numbers with Danish separators', () => {
      const num = 1234.56;
      const result = num.toLocaleString('da-DK');
      // Danish uses dot for thousands and comma for decimals: "1.234,56"
      expect(result).toContain(',');
    });

    test('toLocaleString respects locale parameter', () => {
      const num = 1234.56;
      const da = num.toLocaleString('da-DK');
      const en = num.toLocaleString('en-US');
      // Danish: "1.234,56" vs English: "1,234.56"
      expect(da).not.toBe(en);
    });
  });
});
