import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title="React Native testing docs"
      description="Reference docs for hermes-test">
      <main>
        <section className={styles.hero}>
          <div className="container">
            <p className={styles.kicker}>Built with Hermes engine parity</p>
            <Heading as="h1" className={styles.title}>
              {siteConfig.title}: fast, easy, Jest-API friendly.
            </Heading>
            <p className={styles.subtitle}>{siteConfig.tagline}</p>
            <div className={styles.buttons}>
              <Link className="button button--primary button--lg" to="/docs/getting-started/introduction">
                Start here
              </Link>
              <Link className="button button--secondary button--lg" to="/docs/api">
                API reference
              </Link>
              <Link className={`button button--lg ${styles.ghostButton}`} to="/docs/test-examples/difficult-mock">
                Real test examples
              </Link>
            </div>
            <div className={styles.stats}>
              <span>23x faster full suite</span>
              <span>64x faster cached run</span>
              <span>~350ms watch reruns</span>
            </div>
            <div className={styles.featureBadges}>
              <span
                className={styles.badge}
                data-tip="Scans dependencies and externalizes native modules automatically when possible.">
                Auto native module detection
              </span>
              <span
                className={styles.badge}
                data-tip="Rust CLI orchestration keeps startup and execution path lean.">
                Rust CLI
              </span>
              <span
                className={styles.badge}
                data-tip="Single-pass esbuild bundling for fast deterministic test bundles.">
                esbuild bundling
              </span>
              <span
                className={styles.badge}
                data-tip="Tests execute in Hermes for engine parity with React Native apps.">
                Hermes runtime
              </span>
              <span
                className={styles.badge}
                data-tip="Bundle is compiled and reused with bytecode + cache artifacts for speed.">
                Bytecode processing & caching
              </span>
            </div>
            <div className={styles.demoWrap}>
              <img
                src="/hermes-test/img/demo.gif"
                alt="hermes-test demo"
                className={styles.demo}
              />
            </div>
          </div>
        </section>

        <section className={styles.section}>
          <div className="container">
            <div className="row">
              <div className="col col--4">
                <div className={styles.card}>
                  <Heading as="h3">Auto mock detection</Heading>
                  <p>Mock paths are discovered and wired into runtime interception.</p>
                </div>
              </div>
              <div className="col col--4">
                <div className={styles.card}>
                  <Heading as="h3">Shallow component mock</Heading>
                  <p>
                    Use <code>ht.shallow()</code> and focused overrides for complex component trees.
                  </p>
                </div>
              </div>
              <div className="col col--4">
                <div className={styles.card}>
                  <Heading as="h3">Shims built in or custom</Heading>
                  <p>Use built-in shims or map your own modules in config.</p>
                </div>
              </div>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
