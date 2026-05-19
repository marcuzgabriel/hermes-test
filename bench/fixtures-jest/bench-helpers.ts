// Same functions as expo-app/src/bench-helpers.ts — duplicated for Jest isolation
export function add(a: number, b: number) { return a + b; }
export function sub(a: number, b: number) { return a - b; }
export function mul(a: number, b: number) { return a * b; }
export function div(a: number, b: number) { return b === 0 ? 0 : a / b; }
export function mod(a: number, b: number) { return a % b; }
export function pow(a: number, b: number) { return Math.pow(a, b); }
export function clamp(n: number, min: number, max: number) { return Math.min(Math.max(n, min), max); }
export function lerp(a: number, b: number, t: number) { return a + (b - a) * t; }
export function abs(n: number) { return Math.abs(n); }
export function sign(n: number) { return Math.sign(n); }
export function isEven(n: number) { return n % 2 === 0; }
export function isOdd(n: number) { return n % 2 !== 0; }
export function factorial(n: number): number { return n <= 1 ? 1 : n * factorial(n - 1); }
export function gcd(a: number, b: number): number { return b === 0 ? a : gcd(b, a % b); }
export function lcm(a: number, b: number) { return (a * b) / gcd(a, b); }
export function fibonacci(n: number) { let a = 0, b = 1; for (let i = 0; i < n; i++) { [a, b] = [b, a + b]; } return a; }
export function isPrime(n: number) { if (n < 2) return false; for (let i = 2; i * i <= n; i++) { if (n % i === 0) return false; } return true; }
export function reverse(s: string) { return s.split('').reverse().join(''); }
export function isPalindrome(s: string) { return s === reverse(s); }
export function capitalize(s: string) { return s.charAt(0).toUpperCase() + s.slice(1); }
export function camelCase(s: string) { return s.replace(/[-_\s]+(.)/g, (_: any, c: string) => c.toUpperCase()); }
export function snakeCase(s: string) { return s.replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, ''); }
export function repeat(s: string, n: number) { return s.repeat(n); }
export function truncate(s: string, len: number) { return s.length <= len ? s : s.slice(0, len - 3) + '...'; }
export function unique<T>(arr: T[]) { return [...new Set(arr)]; }
export function sum(arr: number[]) { return arr.reduce((a, b) => a + b, 0); }
export function mean(arr: number[]) { return arr.length === 0 ? 0 : sum(arr) / arr.length; }
export function max(arr: number[]) { return Math.max(...arr); }
export function min(arr: number[]) { return Math.min(...arr); }
export function flatten<T>(arr: T[][]) { return arr.reduce((a, b) => a.concat(b), []); }
export function range(start: number, end: number) { const r: number[] = []; for (let i = start; i < end; i++) r.push(i); return r; }
export function chunk<T>(arr: T[], size: number) { const r: T[][] = []; for (let i = 0; i < arr.length; i += size) r.push(arr.slice(i, i + size)); return r; }
export function zip<A, B>(a: A[], b: B[]) { return a.slice(0, Math.min(a.length, b.length)).map((v, i) => [v, b[i]] as [A, B]); }
export function compact<T>(arr: (T | null | undefined | false | 0 | '')[]): T[] { return arr.filter(Boolean) as T[]; }
export function intersection<T>(a: T[], b: T[]) { const s = new Set(b); return a.filter(x => s.has(x)); }
