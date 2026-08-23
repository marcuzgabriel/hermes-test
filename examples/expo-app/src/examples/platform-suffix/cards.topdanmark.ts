// A code file whose name contains a dot before the real extension — the React Native platform /
// brand-suffix convention (`Button.ios.tsx`, `api.native.ts`, `cards.topdanmark.ts`). Imported
// WITHOUT the extension, so the specifier `./cards.topdanmark` looks like it ends in ".topdanmark".
export function cardsFor(brand: string): string[] {
  return [`${brand}-card-1`, `${brand}-card-2`];
}
