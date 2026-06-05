import React, { useState } from 'react';
import { View, Text, Pressable, TextInput } from 'react-native';
import { test, expect, render, fireEvent, group } from 'hermes-test';

// --- Test components ---

function Greeting({ name }: { name: string }) {
  return (
    <View testID="greeting">
      <Text>Hello, {name}!</Text>
    </View>
  );
}

function Counter() {
  const [count, setCount] = useState(0);
  return (
    <View testID="counter">
      <Text testID="count">{String(count)}</Text>
      <Pressable testID="increment" onPress={() => setCount(c => c + 1)}>
        <Text>+</Text>
      </Pressable>
      <Pressable testID="decrement" onPress={() => setCount(c => c - 1)}>
        <Text>-</Text>
      </Pressable>
    </View>
  );
}

function SearchBox({ onSearch }: { onSearch: (q: string) => void }) {
  const [query, setQuery] = useState('');
  return (
    <View>
      <TextInput
        testID="search-input"
        value={query}
        onChangeText={(t: string) => { setQuery(t); }}
      />
      <Pressable testID="search-btn" onPress={() => onSearch(query)}>
        <Text>Search</Text>
      </Pressable>
    </View>
  );
}

function StyledBox() {
  return (
    <View testID="box" style={{ backgroundColor: 'red', padding: 10, opacity: 1 }}>
      <Text style={[{ fontSize: 16 }, { color: 'white' }]}>Styled</Text>
    </View>
  );
}

function DisabledButton() {
  return (
    <View>
      <Pressable testID="enabled-btn" onPress={() => {}}>
        <Text>Enabled</Text>
      </Pressable>
      <Pressable testID="disabled-btn" disabled={true} onPress={() => {}}>
        <Text>Disabled</Text>
      </Pressable>
    </View>
  );
}

function SearchInput() {
  const [value, setValue] = useState('initial');
  return (
    <TextInput
      testID="input"
      value={value}
      onChangeText={setValue}
      placeholder="Type here"
    />
  );
}

// --- render + queries ---

group('render', () => {
  test('getByText finds text', () => {
    const { getByText } = render(<Greeting name="world" />);
    expect(getByText('Hello, world!')).toBeTruthy();
  });

  test('getByTestId finds element', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    const el = getByTestId('greeting');
    expect(el.type).toBe('View');
  });

  test('queryByText returns null for missing', () => {
    const { queryByText } = render(<Greeting name="world" />);
    expect(queryByText('Goodbye')).toBeNull();
  });

  test('toJSON produces snapshot', () => {
    const { toJSON } = render(<Greeting name="world" />);
    const json = toJSON();
    expect(json.type).toBe('View');
    expect(json.props.testID).toBe('greeting');
    expect(json.children).toHaveLength(1);
  });

  test('toTree produces readable output', () => {
    const { toTree } = render(<Greeting name="world" />);
    const tree = toTree();
    expect(tree).toContain('View');
    expect(tree).toContain('Text');
    expect(tree).toContain('Hello, world!');
  });

  test('rerender updates tree', () => {
    const { getByText, rerender } = render(<Greeting name="world" />);
    expect(getByText('Hello, world!')).toBeTruthy();
    rerender(<Greeting name="hermes" />);
    expect(getByText('Hello, hermes!')).toBeTruthy();
  });

  test('unmount clears tree', () => {
    const r = render(<Greeting name="world" />);
    expect(r.toJSON()).toBeTruthy();
    r.unmount();
    expect(r.toJSON()).toBeNull();
  });
});

// --- fireEvent ---

group('fireEvent', () => {
  test('press updates state', () => {
    const { getByTestId } = render(<Counter />);
    expect(getByTestId('count').children[0].text).toBe('0');

    fireEvent.press(getByTestId('increment'));
    expect(getByTestId('count').children[0].text).toBe('1');

    fireEvent.press(getByTestId('increment'));
    expect(getByTestId('count').children[0].text).toBe('2');

    fireEvent.press(getByTestId('decrement'));
    expect(getByTestId('count').children[0].text).toBe('1');
  });

  test('changeText + press', () => {
    const onSearch = (() => {
      let lastCall: string | undefined;
      const fn = (q: string) => { lastCall = q; };
      fn.lastCall = () => lastCall;
      return fn;
    })();

    const { getByTestId } = render(<SearchBox onSearch={onSearch} />);
    fireEvent.changeText(getByTestId('search-input'), 'react native');
    fireEvent.press(getByTestId('search-btn'));
    expect(onSearch.lastCall()).toBe('react native');
  });
});

// --- Element matchers ---

group('element matchers', () => {
  test('toBeRendered', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).toBeRendered();
    expect(null).not.toBeRendered();
  });

  test('toHaveTextContent: exact', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).toHaveTextContent('Hello, world!');
  });

  test('toHaveTextContent: partial', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).toHaveTextContent('world');
  });

  test('toHaveTextContent: regex', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).toHaveTextContent(/hello/i);
  });

  test('toHaveTextContent: negated', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).not.toHaveTextContent('Goodbye');
  });

  test('toContainElement', () => {
    const { getByTestId, getByText } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).toContainElement(getByText('Hello, world!'));
  });

  test('toContainElement: negated', () => {
    const { getByTestId } = render(
      <View>
        <View testID="a"><Text>A</Text></View>
        <View testID="b"><Text>B</Text></View>
      </View>
    );
    expect(getByTestId('a')).not.toContainElement(getByTestId('b'));
  });

  test('toBeEmpty', () => {
    const { getByTestId } = render(<View testID="empty" />);
    expect(getByTestId('empty')).toBeEmpty();
  });

  test('toBeEmpty: negated', () => {
    const { getByTestId } = render(<Greeting name="world" />);
    expect(getByTestId('greeting')).not.toBeEmpty();
  });

  test('toHaveDisplayValue', () => {
    const { getByTestId } = render(<SearchInput />);
    expect(getByTestId('input')).toHaveDisplayValue('initial');
    expect(getByTestId('input')).toHaveDisplayValue(/init/);
  });

  test('toHaveDisplayValue: after change', () => {
    const { getByTestId } = render(<SearchInput />);
    fireEvent.changeText(getByTestId('input'), 'updated');
    expect(getByTestId('input')).toHaveDisplayValue('updated');
  });

  test('toHaveProp', () => {
    const { getByTestId } = render(<SearchInput />);
    expect(getByTestId('input')).toHaveProp('placeholder');
    expect(getByTestId('input')).toHaveProp('placeholder', 'Type here');
    expect(getByTestId('input')).not.toHaveProp('disabled');
  });

  test('toHaveStyle: single', () => {
    const { getByTestId } = render(<StyledBox />);
    expect(getByTestId('box')).toHaveStyle({ backgroundColor: 'red' });
  });

  test('toHaveStyle: multiple', () => {
    const { getByTestId } = render(<StyledBox />);
    expect(getByTestId('box')).toHaveStyle({ backgroundColor: 'red', padding: 10 });
  });

  test('toHaveStyle: flattened array', () => {
    const { getByText } = render(<StyledBox />);
    expect(getByText('Styled')).toHaveStyle({ fontSize: 16, color: 'white' });
  });

  test('toHaveStyle: negated', () => {
    const { getByTestId } = render(<StyledBox />);
    expect(getByTestId('box')).not.toHaveStyle({ backgroundColor: 'blue' });
  });

  test('toBeEnabled / toBeDisabled', () => {
    const { getByTestId } = render(<DisabledButton />);
    expect(getByTestId('enabled-btn')).toBeEnabled();
    expect(getByTestId('disabled-btn')).toBeDisabled();
    expect(getByTestId('disabled-btn')).not.toBeEnabled();
  });

  test('toBeVisible', () => {
    const { getByTestId } = render(<StyledBox />);
    expect(getByTestId('box')).toBeVisible();
  });

  test('toBeVisible: display none', () => {
    const { getByTestId } = render(
      <View testID="hidden" style={{ display: 'none' }}><Text>Hidden</Text></View>
    );
    expect(getByTestId('hidden')).not.toBeVisible();
  });

  test('toBeVisible: opacity 0', () => {
    const { getByTestId } = render(
      <View testID="transparent" style={{ opacity: 0 }}><Text>Ghost</Text></View>
    );
    expect(getByTestId('transparent')).not.toBeVisible();
  });
});
