---
name: reactive-sqlite-ui
description: Build SQLite-backed reactive UI in `apps/desktop` using stable patterns for reads, selection, forms, writes, and loading states. Use when implementing or reviewing screens built on `useDrizzleLiveQuery` and SQLite mutations.
metadata:
  internal: true
---

## Goal

Use SQLite live queries as the screen data source without duplicating subscriptions, fighting the reactivity model, or making selection and form state unstable.

## Patterns

### 1. Screen Boundary Query

- Subscribe once per resource domain at the screen boundary.
- Pass derived data downward.
- Open another live query only when the child truly needs different data.

```ts
const items = useItems();
const selectedItem = items.find((item) => item.id === selectedId) ?? null;
```

### 2. Selection Is UI State

- Keep selection in tab/zustand/local UI state as an id or index.
- Do not model selection as a second live query.
- Resolve the selected item from the already-loaded collection when possible.

### 3. Detail Pane From Snapshot

- Pass the selected object into detail components.
- Key detail forms by entity id only when you want local state reset on entity switch.
- Preserve visible data across normal selection changes.

This is the main anti-flicker pattern.

### 4. Forms Own Draft State

- Reactive query data is the source for persisted state.
- The form owns draft state while editing.
- Reset form state when the entity changes, not on every reactive payload.

### 5. Writes Flow One Way

- Reads: `useDrizzleLiveQuery(db.select()...)`
- Imperative reads: helper/query outside the render path
- Writes: `db.insert`, `db.update`, `db.delete` inside `useMutation`
- Let SQLite change notifications update subscribed UI

Do not add manual invalidation unless the write affects data outside the subscribed query graph.

### 6. Loading States Are Scoped

- Show loading UI for initial screen load.
- Do not blank the detail pane for ordinary selection changes.
- Prefer preserving previous data until the next snapshot when query args change frequently.

## Anti-Patterns

- Parent list query plus child selected-row query for the same table
- Child components that take `id` and immediately subscribe again
- Resetting detail data to `undefined` during normal selection changes
- Form resets tied to every incoming reactive payload
- Mixing imperative local cache invalidation with SQLite live-query updates

## Review Check

- Is there more than one live query for the same resource on the same screen?
- Can the detail view render from the parent snapshot?
- Does entity change reset the form, while ordinary reactive updates do not?
- Is loading UI limited to initial load or truly missing data?
- Are writes relying on the live-query loop instead of manual sync?
