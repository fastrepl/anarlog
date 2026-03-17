type Sortable = {
  id: string;
  disabled?: boolean;
  displayName: string;
};

export function sortProviders<T extends Sortable>(
  providers: readonly T[],
): T[] {
  return [...providers].sort((a, b) => {
    if (a.id === "hyprnote") return -1;
    if (b.id === "hyprnote") return 1;

    if (a.disabled && !b.disabled) return 1;
    if (!a.disabled && b.disabled) return -1;

    if (a.id === "custom") return 1;
    if (b.id === "custom") return -1;

    const localOnlyIds = ["ollama", "lmstudio"];
    const aIsLocalOnly = localOnlyIds.includes(a.id);
    const bIsLocalOnly = localOnlyIds.includes(b.id);
    if (aIsLocalOnly && !bIsLocalOnly) return 1;
    if (!aIsLocalOnly && bIsLocalOnly) return -1;

    return a.displayName.localeCompare(b.displayName);
  });
}
