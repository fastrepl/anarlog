export type RailCardInput = {
  id: string;
  desiredTop: number;
  height: number;
};

export type RailCardPlacement = {
  id: string;
  top: number;
};

/**
 * Stacks comment cards in the rail without overlap. Cards are ordered by
 * their anchor's vertical position; the active card is pinned exactly at its
 * desired top, and neighbors are pushed up/down around it with `gap` spacing.
 */
export function layoutRailCards(
  cards: readonly RailCardInput[],
  options: { gap: number; activeId: string | null },
): RailCardPlacement[] {
  if (cards.length === 0) return [];
  const { gap, activeId } = options;
  const ordered = [...cards].sort(
    (left, right) =>
      left.desiredTop - right.desiredTop || left.id.localeCompare(right.id),
  );

  const activeIndex = activeId
    ? ordered.findIndex((card) => card.id === activeId)
    : -1;

  const tops = new Array<number>(ordered.length);
  if (activeIndex === -1) {
    let cursor = -Infinity;
    ordered.forEach((card, index) => {
      const top = Math.max(card.desiredTop, cursor);
      tops[index] = top;
      cursor = top + card.height + gap;
    });
  } else {
    tops[activeIndex] = Math.max(0, ordered[activeIndex].desiredTop);

    let above = tops[activeIndex];
    for (let index = activeIndex - 1; index >= 0; index -= 1) {
      const card = ordered[index];
      const top = Math.min(card.desiredTop, above - gap - card.height);
      tops[index] = top;
      above = top;
    }

    let cursor = tops[activeIndex] + ordered[activeIndex].height + gap;
    for (let index = activeIndex + 1; index < ordered.length; index += 1) {
      const card = ordered[index];
      const top = Math.max(card.desiredTop, cursor);
      tops[index] = top;
      cursor = top + card.height + gap;
    }
  }

  const shift = Math.min(...tops);
  const normalized = shift < 0 ? tops.map((top) => top - shift) : tops;
  return ordered.map((card, index) => ({
    id: card.id,
    top: normalized[index],
  }));
}

/** Smallest range wins when several highlights overlap a click. */
export function pickActiveCommentId(
  candidates: ReadonlyArray<{ commentId: string; from: number; to: number }>,
  commentIds: readonly string[],
): string | null {
  const eligible = candidates
    .filter((candidate) => commentIds.includes(candidate.commentId))
    .sort(
      (left, right) =>
        left.to - left.from - (right.to - right.from) ||
        left.from - right.from ||
        left.commentId.localeCompare(right.commentId),
    );
  return eligible[0]?.commentId ?? null;
}
