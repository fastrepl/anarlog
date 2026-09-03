import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./render-outlines", () => ({
  flashRenderOutlines: vi.fn(),
  hideRenderOutlines: vi.fn(),
}));

const hook = vi.hoisted(() => ({
  listeners: new Set<(root: unknown) => void>(),
}));

vi.mock("./render-hook", () => ({
  onReactCommit: (listener: (root: unknown) => void) => {
    hook.listeners.add(listener);
    return () => hook.listeners.delete(listener);
  },
}));

import type { Fiber, FiberRoot } from "./render-hook";
import { flashRenderOutlines, hideRenderOutlines } from "./render-outlines";
import {
  areRenderOutlinesEnabled,
  getDisplayName,
  getTopRenderedComponents,
  ignoreRenderTracking,
  setRenderOutlinesEnabled,
  startRenderTracker,
  tickRenderTracker,
  traverseRenderedFibers,
} from "./render-tracker";

const FunctionComponent = 0;
const HostRoot = 3;
const HostComponent = 5;
const PerformedWork = 0b1;

function fiber(overrides: Partial<Fiber> & { tag: number }): Fiber {
  return {
    type: null,
    flags: 0,
    memoizedProps: {},
    memoizedState: null,
    stateNode: null,
    alternate: null,
    child: null,
    sibling: null,
    ...overrides,
  };
}

function component(
  name: string,
  flags = PerformedWork,
  child: Fiber | null = null,
) {
  const type = { [name]: () => null }[name]!;
  return fiber({ tag: FunctionComponent, type, flags, child });
}

function host(child: Fiber | null = null) {
  return fiber({
    tag: HostComponent,
    type: "div",
    stateNode: document.createElement("div"),
    child,
  });
}

function root(child: Fiber, alternate: Fiber | null = null): FiberRoot {
  const current = fiber({
    tag: HostRoot,
    child,
    alternate,
    memoizedState: { element: {} },
  });
  return { current };
}

function collectNames(fiberRoot: FiberRoot) {
  const names: string[] = [];
  traverseRenderedFibers(fiberRoot, (rendered) =>
    names.push(getDisplayName(rendered.type)),
  );
  return names;
}

describe("traverseRenderedFibers", () => {
  it("reports every composite fiber on initial mount", () => {
    const leaf = component("Leaf");
    const parent = component("Parent", PerformedWork, host(leaf));
    parent.child!.sibling = component("Aside");

    expect(collectNames(root(parent))).toEqual(["Parent", "Leaf", "Aside"]);
  });

  it("skips subtrees React bailed out of during an update", () => {
    const untouchedChild = component("Untouched", 0);
    const previousApp = component("App", 0, untouchedChild);
    const previousRoot = fiber({
      tag: HostRoot,
      child: previousApp,
      memoizedState: { element: {} },
    });

    // App re-rendered but reused its child fiber, so the child was bailed out.
    const nextApp = component("App", PerformedWork, untouchedChild);
    nextApp.alternate = previousApp;

    expect(collectNames(root(nextApp, previousRoot))).toEqual(["App"]);
  });

  it("descends into changed children and freshly mounted subtrees", () => {
    const previousChild = component("Child", 0);
    const previousApp = component("App", 0, previousChild);
    const previousRoot = fiber({
      tag: HostRoot,
      child: previousApp,
      memoizedState: { element: {} },
    });

    const nextChild = component("Child", PerformedWork);
    nextChild.alternate = previousChild;
    const mounted = component("Mounted", PerformedWork, component("Inner"));
    nextChild.sibling = mounted;
    const nextApp = component("App", 0, nextChild);
    nextApp.alternate = previousApp;

    expect(collectNames(root(nextApp, previousRoot))).toEqual([
      "Child",
      "Mounted",
      "Inner",
    ]);
  });

  it("skips marked components together with everything they render", () => {
    const props = { ignored: true };
    const ignored = component("Ignored", PerformedWork, component("Nested"));
    ignored.memoizedProps = props;
    ignored.sibling = component("Visible");
    ignoreRenderTracking(props);

    expect(collectNames(root(ignored))).toEqual(["Visible"]);
  });
});

describe("getDisplayName", () => {
  it("unwraps memo and forwardRef wrappers", () => {
    function Named() {
      return null;
    }
    expect(getDisplayName(Named)).toBe("Named");
    expect(getDisplayName({ type: Named })).toBe("Named");
    expect(getDisplayName({ render: Named })).toBe("Named");
    expect(getDisplayName({ displayName: "Custom", type: Named })).toBe(
      "Custom",
    );
    expect(getDisplayName(null)).toBe("Anonymous");
  });
});

describe("render tracker counters", () => {
  let stop: () => void;

  beforeEach(() => {
    stop = startRenderTracker();
    while (tickRenderTracker() || getTopRenderedComponents().length) {
      // drain buckets left over from other tests
    }
  });

  afterEach(() => {
    stop();
    vi.clearAllMocks();
  });

  function commit(fiberRoot: FiberRoot) {
    hook.listeners.forEach((listener) => listener(fiberRoot));
  }

  it("counts renders per tick and ranks components over the window", () => {
    setRenderOutlinesEnabled(false);
    commit(root(component("Hot", PerformedWork, component("Cold"))));
    commit(root(component("Hot")));

    expect(tickRenderTracker()).toBe(3);
    expect(getTopRenderedComponents()).toEqual([
      { name: "Hot", count: 2 },
      { name: "Cold", count: 1 },
    ]);
    expect(tickRenderTracker()).toBe(0);
    expect(flashRenderOutlines).not.toHaveBeenCalled();
  });

  it("flashes the nearest host nodes when outlines are enabled", () => {
    setRenderOutlinesEnabled(true);
    const node = host();
    commit(root(component("Boxed", PerformedWork, node)));

    expect(areRenderOutlinesEnabled()).toBe(true);
    expect(flashRenderOutlines).toHaveBeenCalledWith([
      { node: node.stateNode, name: "Boxed" },
    ]);

    setRenderOutlinesEnabled(false);
    expect(hideRenderOutlines).toHaveBeenCalled();
  });
});
