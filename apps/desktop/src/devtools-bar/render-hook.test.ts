import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type Hook = {
  renderers: Map<number, unknown>;
  supportsFiber: boolean;
  inject: (renderer: unknown) => number;
  onCommitFiberRoot: (id: number, root: unknown) => void;
};

const scope = globalThis as { __REACT_DEVTOOLS_GLOBAL_HOOK__?: Hook };

describe("render-hook", () => {
  let previousHook: Hook | undefined;

  beforeEach(() => {
    previousHook = scope.__REACT_DEVTOOLS_GLOBAL_HOOK__;
    delete scope.__REACT_DEVTOOLS_GLOBAL_HOOK__;
    vi.resetModules();
  });

  afterEach(() => {
    if (previousHook) {
      scope.__REACT_DEVTOOLS_GLOBAL_HOOK__ = previousHook;
    } else {
      delete scope.__REACT_DEVTOOLS_GLOBAL_HOOK__;
    }
  });

  it("installs a fiber-capable hook and forwards commits to subscribers", async () => {
    const { onReactCommit } = await import("./render-hook");
    const hook = scope.__REACT_DEVTOOLS_GLOBAL_HOOK__!;

    expect(hook.supportsFiber).toBe(true);
    const renderer = { version: "19" };
    const id = hook.inject(renderer);
    expect(hook.renderers.get(id)).toBe(renderer);

    const listener = vi.fn();
    const unsubscribe = onReactCommit(listener);
    const root = { current: null };
    hook.onCommitFiberRoot(id, root);
    expect(listener).toHaveBeenCalledWith(root);

    unsubscribe();
    hook.onCommitFiberRoot(id, root);
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("chains onto an existing hook instead of replacing it", async () => {
    const existingCommit = vi.fn();
    const existing: Hook = {
      renderers: new Map(),
      supportsFiber: true,
      inject: () => 1,
      onCommitFiberRoot: existingCommit,
    };
    scope.__REACT_DEVTOOLS_GLOBAL_HOOK__ = existing;

    const { onReactCommit } = await import("./render-hook");
    const listener = vi.fn();
    onReactCommit(listener);

    expect(scope.__REACT_DEVTOOLS_GLOBAL_HOOK__).toBe(existing);
    const root = { current: null };
    existing.onCommitFiberRoot(1, root);
    expect(existingCommit).toHaveBeenCalledWith(1, root);
    expect(listener).toHaveBeenCalledWith(root);
  });
});
