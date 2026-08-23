// Pattern: asset imports (fonts, media) in the import graph
// Demonstrates: a module that `require()`s a .ttf (like @expo/vector-icons) bundles fine —
// the asset is loaded as `empty`, exactly like .png/.svg already were. Before 1.3.1 one such
// import anywhere in the graph failed the whole bundle with "No loader is configured for .ttf".
import { test, group, expect } from 'hermes-test';
import { iconFont, iconFontName } from './asset-imports/icon-font';

group('asset imports', () => {
  test('a .ttf require() does not break bundling and resolves to an empty module', () => {
    expect(iconFontName()).toBe('Icon');
    // esbuild's `empty` loader yields an empty module object ({}) or undefined depending on interop.
    expect(iconFont === undefined || typeof iconFont === 'object').toBe(true);
  });
});
