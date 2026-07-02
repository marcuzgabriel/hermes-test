// Consumer imports the service through the tsconfig path alias.
import { getServiceGreeting } from '@app/alias-mock/greeting-service';

export function buildAliasSession(): string {
  return `alias session: ${getServiceGreeting()}`;
}
