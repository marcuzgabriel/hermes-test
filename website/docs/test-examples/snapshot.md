---
title: Snapshot test
---

# Snapshot test

Use snapshots to lock UI output shape for stable components.

```tsx
import React from 'react';
import {test, render} from 'hermes-test';
import {Text} from 'react-native';

function Badge({label}: {label: string}) {
  return <Text>{label}</Text>;
}

test('matches badge snapshot', ({expect}) => {
  const screen = render(<Badge label="Premium" />);
  expect(screen.toJSON()).toMatchSnapshot();
});
```

Tip: keep snapshots focused and avoid snapshotting huge trees with unstable data.
