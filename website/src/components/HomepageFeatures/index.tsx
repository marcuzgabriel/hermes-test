import type {ReactNode} from 'react';
import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'Hermes-first runtime',
    description: (
      <>
        Run tests in Hermes with deterministic behavior and no Jest worker
        overhead.
      </>
    ),
  },
  {
    title: 'Clear API reference',
    description: (
      <>
        Quickly find matchers and helpers for <code>test</code>,{' '}
        <code>expect</code>, and <code>mock</code>.
      </>
    ),
  },
  {
    title: 'GitHub-native docs',
    description: (
      <>
        Docs source lives in this repo and deploys automatically with GitHub
        Actions.
      </>
    ),
  },
];

function Feature({title, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
