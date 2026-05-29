import { useRef } from "react";

import { PersistentChatPanel } from "~/chat/components/persistent-chat";

export function MainChatPanels({ children }: { children: React.ReactNode }) {
  const bodyPanelContainerRef = useRef<HTMLDivElement>(null);

  return (
    <>
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <div
          ref={bodyPanelContainerRef}
          className="h-full min-h-0 min-w-0 flex-1 overflow-hidden"
        >
          {children}
        </div>
      </div>

      <PersistentChatPanel floatingContainerRef={bodyPanelContainerRef} />
    </>
  );
}
