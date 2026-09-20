import { LiveAssistCard } from "./card";

import { useLiveAssistCards } from "~/store/zustand/live-assist";

// Cards are newest-first (the store prepends), and the panel renders nothing
// until the first suggestion exists: a proactive feature that only speaks up
// once it has something to say should not add a noisy empty state.
export function LiveAssistPanel({ sessionId }: { sessionId: string }) {
  const cards = useLiveAssistCards(sessionId);

  if (cards.length === 0) {
    return null;
  }

  return (
    <div className="flex h-full flex-col gap-2 overflow-y-auto p-3">
      {cards.map((card) => (
        <LiveAssistCard key={card.id} sessionId={sessionId} card={card} />
      ))}
    </div>
  );
}
