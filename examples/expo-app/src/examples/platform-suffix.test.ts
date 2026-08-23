// Pattern: dotted file names imported without extension
// Demonstrates: `./cards.topdanmark` (a .ts file) must resolve as code — the asset catch-all only
// claims specifiers that name an existing non-code file on disk. Regression guard for 1.5.1,
// where this import became an empty module (`cardsFor` undefined) in a real app.
import { test, group, expect } from 'hermes-test';
import { cardsFor } from './platform-suffix/cards.topdanmark';
import { iconFontName } from './asset-imports/icon-font';

group('dotted file names without extension', () => {
  test('resolves to the code file, not an empty asset module', () => {
    expect(typeof cardsFor).toBe('function');
    expect(cardsFor('topdanmark')).toEqual(['topdanmark-card-1', 'topdanmark-card-2']);
  });

  test('real asset files are still loaded as empty modules', () => {
    expect(iconFontName()).toBe('Icon');
  });
});
