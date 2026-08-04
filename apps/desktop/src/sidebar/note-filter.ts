export type SidebarNoteFilter =
  | "all"
  | "mine"
  | "shared-by-me"
  | "shared-with-me"
  | `workspace:${string}`;

export function getSidebarWorkspaceFilterId(
  filter: SidebarNoteFilter,
): string | null {
  return filter.startsWith("workspace:")
    ? filter.slice("workspace:".length)
    : null;
}

export function sidebarNoteFilterShowsTimeline(
  filter: SidebarNoteFilter,
): boolean {
  return filter === "all" || filter === "mine" || filter === "shared-by-me";
}
