import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'hermes-test',
  tagline: 'Fast, deterministic tests for React Native in Hermes',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://marcuzgabriel.github.io',
  baseUrl: '/hermes-test/',

  organizationName: 'marcuzgabriel',
  projectName: 'hermes-test',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/marcuzgabriel/hermes-test/tree/main/website/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/docusaurus-social-card.jpg',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    tableOfContents: {
      minHeadingLevel: 2,
      maxHeadingLevel: 4,
    },
    navbar: {
      title: 'hermes-test',
      logo: {
        alt: 'hermes-test logo',
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          href: 'https://github.com/marcuzgabriel/hermes-test',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {
              label: 'Getting started',
              to: '/docs/getting-started/introduction',
            },
            {label: 'expect API', to: '/docs/api/expect'},
          ],
        },
        {
          title: 'Architecture',
          items: [
            {label: 'Overview', to: '/docs/architecture/overview'},
            {label: 'Coverage model', to: '/docs/architecture/coverage'},
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/marcuzgabriel/hermes-test',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} hermes-test.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
