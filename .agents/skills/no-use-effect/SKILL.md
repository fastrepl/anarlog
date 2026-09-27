---
name: no-use-effect
description: Avoid direct React `useEffect` usage when writing or reviewing React components and hooks. Use this skill when creating new React code, refactoring existing `useEffect` calls, reviewing PRs that introduce `useEffect`, or when an agent reaches for `useEffect` as a default. Replace effects with derived state, query or mutation libraries, event handlers, `useMountEffect` for mount-only external sync, or `key`-based remounts.
---

# No useEffect

Never call `useEffect` directly unless the work is a true mount-only sync with an external system and no simpler pattern fits. Prefer a render-time computation, a query or mutation hook, an event handler, or a remount boundary instead.

## Workflow

1. Classify why the effect exists.
2. Replace it with the narrowest pattern below.
3. Verify with the project's normal lint, typecheck, and tests.

## Replacement Patterns

### 1. Derive state, do not sync it

Replace `useEffect(() => setX(derive(y)), [y])` with inline computation or the project's existing memoization conventions.

```tsx
const filteredProducts = products.filter((product) => product.inStock);
```

Smell test: state only mirrors props or other state.

### 2. Use query or mutation libraries for async work

Replace fetch-in-effect code with the app's data layer.

```tsx
const productQuery = useQuery({
  queryKey: ["product", productId],
  queryFn: () => fetchProduct(productId),
});
```

Smell test: the effect fetches, awaits, retries, cancels, or writes async results into local state.

### 3. Use event handlers for user actions

Do the work where the action happens instead of setting a flag and reacting later.

```tsx
const handleLike = () => {
  postLike();
};
```

Smell test: state exists only to trigger an effect, then gets reset.

### 4. Use `useMountEffect` for mount-only external sync

Use this only for work that is naturally "set up on mount, clean up on unmount": DOM integration, subscriptions, third-party widgets, and stable singleton bindings.

If the project already has a `useMountEffect`, use it. If not, add a small wrapper:

```tsx
import { useEffect } from "react";

export function useMountEffect(effect: () => void | (() => void)) {
  useEffect(effect, []);
}
```

Prefer changing component structure so the component only mounts once preconditions are satisfied.

Smell test: the effect is a one-time setup or teardown for something outside React.

### 5. Use `key` to reset component state

If a component should behave like a fresh instance when an ID or entity changes, remount it from the parent.

```tsx
return <VideoPlayer key={videoId} videoId={videoId} />;
```

Smell test: the effect exists only to reset local state when props change.

## Review Standard

Treat new direct `useEffect` usage as a bug by default. Ask which of the five patterns applies. Accept direct `useEffect` only with explicit justification that the code is a mount-only external sync and that `useMountEffect` is the intended escape hatch.

## Structure React Code This Way

Order component logic like this:

1. Hooks that read external state or data.
2. Local state.
3. Derived values computed during render.
4. Event handlers.
5. Early returns.
6. Render output.

Do not introduce mirrored state just to make an effect possible.
