import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'index',
    {
      type: 'category',
      label: 'Getting started',
      items: [
        'getting-started/introduction',
        'getting-started/installation',
        'getting-started/first-test',
      ],
    },
    {
      type: 'category',
      label: 'API reference',
      items: [
        'api/index',
        'api/test',
        'api/expect',
        'api/spy',
        'api/mock',
        'api/hooks',
        'api/render',
        'api/store',
        'api/timers',
      ],
    },
    {
      type: 'category',
      label: 'Test examples',
      items: [
        'test-examples/basic-hook',
        'test-examples/redux',
        'test-examples/difficult-mock',
        'test-examples/snapshot',
      ],
    },
    {
      type: 'category',
      label: 'Benchmarks',
      items: ['benchmarks/overview', 'benchmarks/methodology'],
    },
    {
      type: 'category',
      label: 'Architecture',
      items: [
        'architecture/overview',
        'architecture/tooling-rationale',
        'architecture/coverage',
        'architecture/auto-detection',
        'architecture/shims',
      ],
    },
    {
      type: 'category',
      label: 'Issues & platform notes',
      items: ['issues/intl-observations', 'issues/linux-support'],
    },
    {
      type: 'category',
      label: 'References',
      items: [
        'references/index',
        'references/challenges',
        'references/component-rendering',
        'references/coverage-approaches',
        'references/mock-strategies',
        'references/mock-strategy-history',
        'references/performance',
        'references/shadow-wrappers',
        'references/shallow-rendering-fixes',
        'references/todo',
        'references/v8-evaluation-summary',
      ],
    },
  ],
};

export default sidebars;
