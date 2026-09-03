/**
 * Minimal React DevTools global hook so commits can be observed without
 * react-scan. React only reports to a hook that exists before react-dom
 * evaluates, so this module must stay the first import of the app entry.
 * With no listeners attached it costs one no-op call per commit, which is why
 * it is safe to install in every build; all real work is gated behind
 * `onReactCommit` subscribers.
 */

export type Fiber = {
  tag: number;
  type: unknown;
  flags?: number;
  effectTag?: number;
  memoizedProps: unknown;
  memoizedState: unknown;
  stateNode: unknown;
  alternate: Fiber | null;
  child: Fiber | null;
  sibling: Fiber | null;
};

export type FiberRoot = { current: Fiber };

type CommitListener = (root: FiberRoot) => void;

type DevtoolsHook = {
  renderers: Map<number, unknown>;
  supportsFiber: boolean;
  isDisabled?: boolean;
  inject: (renderer: unknown) => number;
  onScheduleFiberRoot?: (...args: unknown[]) => void;
  onCommitFiberRoot: (
    rendererId: number,
    root: FiberRoot,
    ...rest: unknown[]
  ) => void;
  onCommitFiberUnmount: (...args: unknown[]) => void;
  onPostCommitFiberRoot?: (...args: unknown[]) => void;
};

const listeners = new Set<CommitListener>();

function notify(root: FiberRoot) {
  for (const listener of listeners) {
    try {
      listener(root);
    } catch (error) {
      console.error("[devtools-bar] commit listener failed", error);
    }
  }
}

function install() {
  if (typeof globalThis !== "object") return;

  const scope = globalThis as { __REACT_DEVTOOLS_GLOBAL_HOOK__?: DevtoolsHook };
  const existing = scope.__REACT_DEVTOOLS_GLOBAL_HOOK__;

  if (existing) {
    // React Refresh (Vite dev) or a browser extension installed a hook first;
    // chain onto its commit callback instead of replacing it.
    const previous = existing.onCommitFiberRoot;
    existing.onCommitFiberRoot = function chainedOnCommitFiberRoot(
      this: unknown,
      rendererId,
      root,
      ...rest
    ) {
      previous?.call(this, rendererId, root, ...rest);
      if (listeners.size) notify(root);
    };
    return;
  }

  let nextRendererId = 0;
  const renderers = new Map<number, unknown>();

  scope.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    renderers,
    supportsFiber: true,
    inject(renderer) {
      const id = ++nextRendererId;
      renderers.set(id, renderer);
      return id;
    },
    onScheduleFiberRoot() {},
    onCommitFiberRoot(_rendererId, root) {
      if (listeners.size) notify(root);
    },
    onCommitFiberUnmount() {},
    onPostCommitFiberRoot() {},
  };
}

install();

export function onReactCommit(listener: CommitListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
