---
title: render
---

# render API

## `render`

```tsx
import {render} from 'hermes-test';

const screen = render(<MyComponent />);
```

## Query API

All families support `get*`, `getAll*`, `query*`, `queryAll*`:

- `ByText`
- `ByTestId`
- `ByProps`
- `ByType`

## Events

```ts
import {fireEvent} from 'hermes-test';

fireEvent.press(node);
fireEvent.changeText(inputNode, 'new value');
fireEvent.scroll(listNode, {nativeEvent: {contentOffset: {y: 100}}});
fireEvent(node, 'focus');
```

## Serialization and lifecycle

```ts
screen.toJSON();
screen.toTree();
screen.rerender(<MyComponent updated />);
screen.unmount();
```
