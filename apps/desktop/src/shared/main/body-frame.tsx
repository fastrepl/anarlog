import { MainChatPanels } from "./chat-panels";
import {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
} from "./session-status-banner";

import { useShell } from "~/contexts/shell";

export function MainShellBodyFrame({
  autoSaveId,
  children,
}: {
  autoSaveId: string;
  children: React.ReactNode;
}) {
  const { chat } = useShell();

  return (
    <SessionStatusBannerProvider>
      <MainChatPanels
        autoSaveId={autoSaveId}
        isRightPanelOpen={chat.mode === "RightPanelOpen"}
      >
        {children}
      </MainChatPanels>
      <MainSessionStatusBannerHost />
    </SessionStatusBannerProvider>
  );
}
