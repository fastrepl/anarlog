type Sortable = {
  id: string;
  disabled?: boolean;
  displayName: string;
};

export function sortProviders<T extends Sortable>(
  providers: readonly T[],
  preferredOrder: readonly string[],
): T[] {
  const rankById = new Map(
    preferredOrder.map((providerId, index) => [providerId, index]),
  );

  return [...providers].sort((a, b) => {
    if (a.id === "anarlog") return -1;
    if (b.id === "anarlog") return 1;

    if (a.id === "custom") return 1;
    if (b.id === "custom") return -1;

    if (a.disabled && !b.disabled) return 1;
    if (!a.disabled && b.disabled) return -1;

    const aRank = rankById.get(a.id);
    const bRank = rankById.get(b.id);
    if (aRank !== undefined || bRank !== undefined) {
      return (
        (aRank ?? Number.POSITIVE_INFINITY) -
        (bRank ?? Number.POSITIVE_INFINITY)
      );
    }

    return a.displayName.localeCompare(b.displayName);
  });
}
