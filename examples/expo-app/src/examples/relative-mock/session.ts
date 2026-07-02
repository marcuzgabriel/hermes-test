// Consumer imports the flow with a specifier relative to THIS file.
import { getGreeting } from './flows/greeting-flow';

export function buildSession(): string {
  return `session: ${getGreeting()}`;
}
