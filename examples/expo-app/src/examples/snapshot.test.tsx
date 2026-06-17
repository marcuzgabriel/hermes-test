import React from 'react';
import { View, Text } from 'react-native';
import { test, expect, render, group } from 'hermes-test';

function Greeting({ name }: { name: string }) {
  return (
    <View testID="greeting">
      <Text>Hello, {name}!</Text>
    </View>
  );
}

group('snapshot', () => {
  test('toMatchSnapshot creates and compares', () => {
    const { toJSON } = render(<Greeting name="world" />);
    expect(toJSON()).toMatchSnapshot();
  });

  test('multiple snapshots in one test', () => {
    const { toJSON, rerender } = render(<Greeting name="Alice" />);
    expect(toJSON()).toMatchSnapshot();
    rerender(<Greeting name="Bob" />);
    expect(toJSON()).toMatchSnapshot();
  });
});
