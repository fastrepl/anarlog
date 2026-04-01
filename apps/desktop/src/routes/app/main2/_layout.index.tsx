import { createFileRoute } from "@tanstack/react-router";

import { TabContentChat } from "~/chat/tab/tab-content";

export const Route = createFileRoute("/app/main2/_layout/")({
  component: Component,
});

function Component() {
  return (
    <div className="flex h-full overflow-hidden bg-stone-50 p-1">
      <div className="flex h-full flex-1 flex-col">
        <TabContentChat
          tab={{
            type: "chat_support",
            active: true,
            slotId: "main2",
            pinned: false,
            state: { groupId: null, initialMessage: null },
          }}
        />
      </div>
    </div>
  );
}
