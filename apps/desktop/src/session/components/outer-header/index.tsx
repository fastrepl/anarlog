import { ListenButton } from "./listen";
import { MetadataButton } from "./metadata";
import { OverflowButton } from "./overflow";

import type { EditorView } from "~/store/zustand/tabs/schema";

export function OuterHeader({
  sessionId,
  currentView,
  title,
}: {
  sessionId: string;
  currentView: EditorView;
  title?: React.ReactNode;
}) {
  return (
    <div className="w-full pt-1">
      <div className="flex min-w-0 items-center justify-between gap-3">
        {title ? <div className="min-w-0 flex-1">{title}</div> : null}
        <div className="flex shrink-0 items-center">
          <ListenButton sessionId={sessionId} />
          <MetadataButton sessionId={sessionId} />
          <OverflowButton sessionId={sessionId} currentView={currentView} />
        </div>
      </div>
    </div>
  );
}
