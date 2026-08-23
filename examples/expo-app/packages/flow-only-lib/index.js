/**
 * @flow strict-local
 */
import typeof { View } from 'react-native';

export type Props = {| children: any |};

export default function FlowOnlyThing(props: Props): string {
  return 'real-flow-only-lib';
}
