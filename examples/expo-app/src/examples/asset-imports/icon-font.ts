// Mirrors what @expo/vector-icons does: require() a .ttf so the bundler treats it as an asset.
// eslint-disable-next-line @typescript-eslint/no-var-requires
export const iconFont = require('./fonts/Icon.ttf');

export function iconFontName(): string {
  return 'Icon';
}
