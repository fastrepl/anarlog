import { type Fiber, type FiberRoot, onReactCommit } from "./render-hook";
import { flashRenderOutlines, hideRenderOutlines } from "./render-outlines";

const FunctionComponent = 0;
const ClassComponent = 1;
const HostComponent = 5;
const HostText = 6;
const ForwardRef = 11;
const MemoComponent = 14;
const SimpleMemoComponent = 15;
const HostHoistable = 26;
const HostSingleton = 27;
const PerformedWork = 0b1;

const TOP_COMPONENTS_WINDOW_TICKS = 10;

const ignoredProps = new WeakSet<object>();
let pendingRenders = 0;
let buckets: Array<Map<string, number>> = [new Map()];
let outlinesEnabled = import.meta.env.DEV;
let stopTracking: (() => void) | null = null;

/** Call with a component's props during render to keep it out of outlines and counts. */
export function ignoreRenderTracking(props: object): void {
  ignoredProps.add(props);
}

export function startRenderTracker(): () => void {
  if (stopTracking) return stopTracking;

  const unsubscribe = onReactCommit(handleCommit);
  stopTracking = () => {
    unsubscribe();
    hideRenderOutlines();
    stopTracking = null;
  };
  return stopTracking;
}

/** Returns renders since the previous tick and starts a new top-components bucket. */
export function tickRenderTracker(): number {
  const renders = pendingRenders;
  pendingRenders = 0;
  buckets.push(new Map());
  if (buckets.length > TOP_COMPONENTS_WINDOW_TICKS) {
    buckets = buckets.slice(buckets.length - TOP_COMPONENTS_WINDOW_TICKS);
  }
  return renders;
}

export function getTopRenderedComponents(
  limit = 8,
): Array<{ name: string; count: number }> {
  const totals = new Map<string, number>();
  for (const bucket of buckets) {
    for (const [name, count] of bucket) {
      totals.set(name, (totals.get(name) ?? 0) + count);
    }
  }
  return [...totals]
    .map(([name, count]) => ({ name, count }))
    .sort((left, right) => right.count - left.count)
    .slice(0, limit);
}

export function areRenderOutlinesEnabled(): boolean {
  return outlinesEnabled;
}

export function setRenderOutlinesEnabled(enabled: boolean): void {
  outlinesEnabled = enabled;
  if (!enabled) {
    hideRenderOutlines();
  }
}

function handleCommit(root: FiberRoot) {
  const bucket = buckets[buckets.length - 1];
  const outlines: Array<{ node: Element; name: string }> = [];

  traverseRenderedFibers(root, (fiber) => {
    const name = getDisplayName(fiber.type);
    pendingRenders += 1;
    bucket.set(name, (bucket.get(name) ?? 0) + 1);
    if (outlinesEnabled) {
      for (const node of nearestHostNodes(fiber)) {
        outlines.push({ node, name });
      }
    }
  });

  if (outlines.length) {
    flashRenderOutlines(outlines);
  }
}

function isCompositeFiber(fiber: Fiber): boolean {
  switch (fiber.tag) {
    case FunctionComponent:
    case ClassComponent:
    case ForwardRef:
    case MemoComponent:
    case SimpleMemoComponent:
      return true;
    default:
      return false;
  }
}

function isTrackedRender(fiber: Fiber): boolean {
  if (!isCompositeFiber(fiber)) return false;
  const flags = fiber.flags ?? fiber.effectTag ?? 0;
  if ((flags & PerformedWork) === 0) return false;
  const props = fiber.memoizedProps;
  return !(props && typeof props === "object" && ignoredProps.has(props));
}

/**
 * Walks only the part of the tree React actually worked on: a fiber whose
 * `child` is the same object as its alternate's was bailed out wholesale, so
 * its subtree can be skipped. Mirrors how React DevTools diffs commits.
 */
export function traverseRenderedFibers(
  root: FiberRoot,
  onRender: (fiber: Fiber) => void,
): void {
  const current = root.current;
  if (!current) return;

  const previous = current.alternate;
  const wasMounted =
    previous !== null &&
    Boolean((previous.memoizedState as { element?: unknown } | null)?.element);

  if (!previous || !wasMounted) {
    mountFiberRecursively(current.child, onRender);
    return;
  }
  updateFiberRecursively(current, previous, onRender);
}

function mountFiberRecursively(
  firstChild: Fiber | null,
  onRender: (fiber: Fiber) => void,
) {
  let fiber = firstChild;
  while (fiber) {
    if (isTrackedRender(fiber)) onRender(fiber);
    mountFiberRecursively(fiber.child, onRender);
    fiber = fiber.sibling;
  }
}

function updateFiberRecursively(
  next: Fiber,
  previous: Fiber,
  onRender: (fiber: Fiber) => void,
) {
  if (isTrackedRender(next)) onRender(next);
  if (next.child === previous.child) return;

  let child = next.child;
  while (child) {
    if (child.alternate) {
      updateFiberRecursively(child, child.alternate, onRender);
    } else {
      // A freshly mounted subtree: every composite fiber in it rendered.
      if (isTrackedRender(child)) onRender(child);
      mountFiberRecursively(child.child, onRender);
    }
    child = child.sibling;
  }
}

function nearestHostNodes(fiber: Fiber): Element[] {
  const nodes: Element[] = [];
  const stack: Fiber[] = fiber.child ? [fiber.child] : [];

  while (stack.length) {
    const current = stack.pop()!;
    if (
      current.tag === HostComponent ||
      current.tag === HostHoistable ||
      current.tag === HostSingleton
    ) {
      if (current.stateNode instanceof Element) nodes.push(current.stateNode);
    } else if (current.tag !== HostText && current.child) {
      stack.push(current.child);
    }
    if (current.sibling) stack.push(current.sibling);
  }

  return nodes;
}

export function getDisplayName(type: unknown): string {
  if (typeof type === "function") {
    const component = type as { displayName?: string; name?: string };
    return component.displayName || component.name || "Anonymous";
  }
  if (type && typeof type === "object") {
    const wrapper = type as {
      displayName?: string;
      type?: unknown;
      render?: unknown;
    };
    if (wrapper.displayName) return wrapper.displayName;
    if (wrapper.type) return getDisplayName(wrapper.type);
    if (wrapper.render) return getDisplayName(wrapper.render);
  }
  return "Anonymous";
}
