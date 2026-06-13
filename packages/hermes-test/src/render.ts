// render() — component rendering for hermes-test
// Reuses the same react-reconciler + host config from hooks.ts.

import { createReconciler, act } from './hooks';

// --- Tree node types ---

export type HTNode = {
  type: string;
  props: Record<string, any>;
  children: HTNode[];
  text?: string;
};

// --- Tree walking ---

function getAllNodes(root: HTNode): HTNode[] {
  const result: HTNode[] = [];
  function walk(node: HTNode) {
    result.push(node);
    for (const child of node.children) walk(child);
  }
  for (const child of root.children) walk(child);
  return result;
}

function getTextContent(node: HTNode): string {
  if (node.type === '__TEXT__') return node.text || '';
  return node.children.map(getTextContent).join('');
}

// --- Queries ---

function textMatches(content: string, text: string | RegExp): boolean {
  return typeof text === 'string' ? content === text : text.test(content);
}

function queryAllByText(root: HTNode, text: string | RegExp): HTNode[] {
  // Match deepest elements only — if a child also matches, skip the parent
  const all = getAllNodes(root).filter(n => {
    if (n.type === '__TEXT__') return false;
    const content = getTextContent(n);
    return content ? textMatches(content, text) : false;
  });
  // Filter out nodes whose children also appear in the match set
  return all.filter(n => !n.children.some(c => c.type !== '__TEXT__' && all.includes(c)));
}

function queryAllByTestId(root: HTNode, testID: string | RegExp): HTNode[] {
  return getAllNodes(root).filter(n => {
    const id = n.props?.testID;
    if (!id) return false;
    return typeof testID === 'string' ? id === testID : testID.test(id);
  });
}

function queryAllByProps(root: HTNode, props: Record<string, any>): HTNode[] {
  return getAllNodes(root).filter(n => {
    for (const key of Object.keys(props)) {
      if (n.props?.[key] !== props[key]) return false;
    }
    return true;
  });
}

function queryAllByType(root: HTNode, type: string): HTNode[] {
  return getAllNodes(root).filter(n => n.type === type);
}

function makeQuery<T>(queryAll: (root: HTNode, arg: T) => HTNode[], label: string) {
  return {
    getAll(root: HTNode, arg: T): HTNode[] {
      const result = queryAll(root, arg);
      if (result.length === 0) throw new Error(`Unable to find element with ${label}: ${String(arg)}`);
      return result;
    },
    get(root: HTNode, arg: T): HTNode {
      const result = queryAll(root, arg);
      if (result.length === 0) throw new Error(`Unable to find element with ${label}: ${String(arg)}`);
      if (result.length > 1) throw new Error(`Found ${result.length} elements with ${label}: ${String(arg)}`);
      return result[0];
    },
    queryAll(root: HTNode, arg: T): HTNode[] {
      return queryAll(root, arg);
    },
    query(root: HTNode, arg: T): HTNode | null {
      const result = queryAll(root, arg);
      if (result.length > 1) throw new Error(`Found ${result.length} elements with ${label}: ${String(arg)}`);
      return result[0] || null;
    },
  };
}

const textQ = makeQuery(queryAllByText, 'text');
const testIdQ = makeQuery(queryAllByTestId, 'testID');
const propsQ = makeQuery(queryAllByProps, 'props');
const typeQ = makeQuery(queryAllByType, 'type');

// --- toJSON serialization ---

function toJSON(node: HTNode): any {
  if (node.type === '__TEXT__') return node.text || '';
  const children = node.children.map(toJSON);
  const cleanProps: Record<string, any> = {};
  for (const [k, v] of Object.entries(node.props || {})) {
    if (typeof v === 'function') {
      cleanProps[k] = '[Function]';
    } else {
      cleanProps[k] = v;
    }
  }
  return {
    type: node.type,
    props: Object.keys(cleanProps).length > 0 ? cleanProps : undefined,
    children: children.length > 0 ? children : undefined,
  };
}

function prettyPrint(json: any, indent: number = 0): string {
  const pad = '  '.repeat(indent);
  if (typeof json === 'string') return `${pad}${json}`;
  const { type, props, children } = json;
  let propsStr = '';
  if (props) {
    const entries = Object.entries(props).map(([k, v]) =>
      typeof v === 'string' ? `${k}="${v}"` : `${k}={${JSON.stringify(v)}}`
    );
    if (entries.length > 0) propsStr = ' ' + entries.join(' ');
  }
  if (!children || children.length === 0) {
    return `${pad}<${type}${propsStr} />`;
  }
  // Collapse adjacent string children into one
  const merged: any[] = [];
  for (const c of children) {
    if (typeof c === 'string' && merged.length > 0 && typeof merged[merged.length - 1] === 'string') {
      merged[merged.length - 1] += c;
    } else {
      merged.push(c);
    }
  }
  // Single text child: inline
  if (merged.length === 1 && typeof merged[0] === 'string') {
    return `${pad}<${type}${propsStr}>${merged[0]}</${type}>`;
  }
  const childrenStr = merged.map((c: any) => prettyPrint(c, indent + 1)).join('\n');
  return `${pad}<${type}${propsStr}>\n${childrenStr}\n${pad}</${type}>`;
}

// --- fireEvent ---

export const fireEvent = Object.assign(
  function fireEvent(node: HTNode, eventName: string, ...args: any[]) {
    const handlerName = 'on' + eventName.charAt(0).toUpperCase() + eventName.slice(1);
    const handler = node.props?.[handlerName];
    if (!handler) throw new Error(`No handler "${handlerName}" on <${node.type}>`);
    act(() => { handler(...args); });
  },
  {
    press(node: HTNode, event?: any) {
      const handler = node.props?.onPress;
      if (!handler) throw new Error(`No "onPress" handler on <${node.type}>`);
      act(() => { handler(event); });
    },
    changeText(node: HTNode, text: string) {
      const handler = node.props?.onChangeText;
      if (!handler) throw new Error(`No "onChangeText" handler on <${node.type}>`);
      act(() => { handler(text); });
    },
    scroll(node: HTNode, event: any) {
      const handler = node.props?.onScroll;
      if (!handler) throw new Error(`No "onScroll" handler on <${node.type}>`);
      act(() => { handler(event); });
    },
  }
);

// --- render() ---

export type RenderResult = {
  container: HTNode;
  getByText(text: string | RegExp): HTNode;
  getAllByText(text: string | RegExp): HTNode[];
  queryByText(text: string | RegExp): HTNode | null;
  queryAllByText(text: string | RegExp): HTNode[];
  getByTestId(testID: string | RegExp): HTNode;
  getAllByTestId(testID: string | RegExp): HTNode[];
  queryByTestId(testID: string | RegExp): HTNode | null;
  queryAllByTestId(testID: string | RegExp): HTNode[];
  getByProps(props: Record<string, any>): HTNode;
  getAllByProps(props: Record<string, any>): HTNode[];
  queryByProps(props: Record<string, any>): HTNode | null;
  queryAllByProps(props: Record<string, any>): HTNode[];
  getByType(type: string): HTNode;
  getAllByType(type: string): HTNode[];
  queryByType(type: string): HTNode | null;
  queryAllByType(type: string): HTNode[];
  toJSON(): any;
  toTree(): string;
  rerender(element: any): void;
  unmount(): void;
};

export function render(element: any, options?: { shallow?: boolean }): RenderResult {
  const reconciler = createReconciler();

  const container: HTNode = { type: '__ROOT__', props: {}, children: [] };
  const root = reconciler.createContainer(
    container,
    0,    // LegacyRoot
    null, // hydrationCallbacks
    false, false, '',
    (err: any) => { throw err; },
    (err: any) => { throw err; },
    null,
    () => {},
  );

  const React = (globalThis as any).__HT_React;

  if (options?.shallow && React) {
    // Shallow rendering: patch React.createElement so child function components
    // become host elements (strings) instead of being called by the reconciler.
    // The top-level component still renders fully (hooks run, JSX returned),
    // but its children are stubbed — no deep import chains, no native module crashes.
    const topType = element.type;
    const origCE = React.createElement;

    React.createElement = function(type: any, ...args: any[]) {
      if (typeof type === 'function' && type !== topType) {
        const name = type.displayName || type.name || 'Component';
        return origCE.call(React, name, ...args);
      }
      return origCE.call(React, type, ...args);
    };

    act(() => {
      reconciler.updateContainer(element, root, null, null);
    });

    React.createElement = origCE;
  } else {
    act(() => {
      reconciler.updateContainer(element, root, null, null);
    });
  }

  const result: RenderResult = {
    container,
    getByText: (t) => textQ.get(container, t),
    getAllByText: (t) => textQ.getAll(container, t),
    queryByText: (t) => textQ.query(container, t),
    queryAllByText: (t) => textQ.queryAll(container, t),
    getByTestId: (id) => testIdQ.get(container, id),
    getAllByTestId: (id) => testIdQ.getAll(container, id),
    queryByTestId: (id) => testIdQ.query(container, id),
    queryAllByTestId: (id) => testIdQ.queryAll(container, id),
    getByProps: (p) => propsQ.get(container, p),
    getAllByProps: (p) => propsQ.getAll(container, p),
    queryByProps: (p) => propsQ.query(container, p),
    queryAllByProps: (p) => propsQ.queryAll(container, p),
    getByType: (t) => typeQ.get(container, t),
    getAllByType: (t) => typeQ.getAll(container, t),
    queryByType: (t) => typeQ.query(container, t),
    queryAllByType: (t) => typeQ.queryAll(container, t),
    toJSON() {
      if (container.children.length === 0) return null;
      if (container.children.length === 1) return toJSON(container.children[0]);
      return container.children.map(toJSON);
    },
    toTree() {
      const json = result.toJSON();
      if (json === null) return '';
      if (Array.isArray(json)) return json.map((j: any) => prettyPrint(j)).join('\n');
      return prettyPrint(json);
    },
    rerender(el: any) {
      act(() => {
        reconciler.updateContainer(el, root, null, null);
      });
    },
    unmount() {
      act(() => {
        reconciler.updateContainer(null, root, null, null);
      });
    },
  };

  return result;
}
