import { ListenButton } from "./listen";
import { MetadataButton } from "./metadata";
import { OverflowButton } from "./overflow";

import type { EditorView } from "~/store/zustand/tabs/schema";

export function OuterHeader({
  sessionId,
  currentView,
}: {
  sessionId: string;
  currentView: EditorView;
}) {
  return (
    <div className="w-full pt-1">
      <div className="flex items-center justify-end">
        <div className="flex shrink-0 items-center">
          <ListenButton sessionId={sessionId} />
          <MetadataButton sessionId={sessionId} />
          <OverflowButton sessionId={sessionId} currentView={currentView} />
        </div>
      </div>
    </div>
  );
}
