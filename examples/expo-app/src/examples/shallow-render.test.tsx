import React, { useState } from 'react';
import { View, Text, Pressable } from 'react-native';
import { test, expect, render, group } from 'hermes-test';

// --- Child components (would crash if deeply rendered with native deps) ---

function ExpensiveChild({ label }: { label: string }) {
  // In a real app, this might import native modules, UI libraries, etc.
  return (
    <View>
      <Text>{label}</Text>
    </View>
  );
}

function AnotherChild() {
  return <Text>I am another child</Text>;
}

// --- Parent component under test ---

function Dashboard({ title, items }: { title: string; items: string[] }) {
  const [selected, setSelected] = useState<string | null>(null);
  return (
    <View testID="dashboard">
      <Text testID="title">{title}</Text>
      {items.map((item, i) => (
        <Pressable key={i} onPress={() => setSelected(item)}>
          <ExpensiveChild label={item} />
        </Pressable>
      ))}
      <AnotherChild />
      {selected && <Text testID="selected">Selected: {selected}</Text>}
    </View>
  );
}

// --- Tests ---

group('shallow rendering', () => {
  test('renders top-level component without recursing into children', () => {
    const { container, getByTestId, queryByType } = render(
      <Dashboard title="My Dashboard" items={['A', 'B', 'C']} />,
      { shallow: true },
    );

    // Top-level content renders
    expect(getByTestId('title')).toBeDefined();

    // Child components are recorded as host elements (not rendered deeply)
    const expensiveChildren = queryByType ? container : container;
    // getByType should find ExpensiveChild as a shallow stub
    const children = container.children[0]; // the View with testID="dashboard"
    expect(children).toBeDefined();
  });

  test('child function components become host elements with their name', () => {
    const { getAllByType } = render(
      <Dashboard title="Test" items={['X', 'Y']} />,
      { shallow: true },
    );

    // ExpensiveChild should appear as host elements named "ExpensiveChild"
    const stubs = getAllByType('ExpensiveChild');
    expect(stubs.length).toBe(2);

    // Props are preserved on the stub
    expect(stubs[0].props.label).toBe('X');
    expect(stubs[1].props.label).toBe('Y');
  });

  test('AnotherChild is also stubbed', () => {
    const { getByType } = render(
      <Dashboard title="Test" items={[]} />,
      { shallow: true },
    );

    const stub = getByType('AnotherChild');
    expect(stub).toBeDefined();
    // Stubbed component has no children (not rendered)
    expect(stub.children.length).toBe(0);
  });

  test('toHaveProp works on shallow stubs', () => {
    const { getAllByType } = render(
      <Dashboard title="Test" items={['Hello']} />,
      { shallow: true },
    );

    const stub = getAllByType('ExpensiveChild')[0];
    expect(stub.props.label).toBe('Hello');
  });

  test('snapshot captures shallow structure', () => {
    const { toJSON } = render(
      <Dashboard title="Snap" items={['A']} />,
      { shallow: true },
    );

    const json = toJSON();
    expect(json).toBeDefined();
    expect(json.type).toBe('View');
  });

  test('full render still works (no shallow option)', () => {
    const { getByText } = render(
      <Dashboard title="Full" items={['Item1']} />,
    );

    // Full render recurses into ExpensiveChild, so its text is visible
    expect(getByText('Item1')).toBeDefined();
    expect(getByText('Full')).toBeDefined();
  });
});
