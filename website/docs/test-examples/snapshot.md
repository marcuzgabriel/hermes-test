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

Snapshots are stored next to the test file that registered them:
`<dir>/__snapshots__/<test-file-name>.snap`, keyed by `group > test name`. Update with
`hermes-test --update-snapshots`.

Tip: keep snapshots focused and avoid snapshotting huge trees with unstable data.
