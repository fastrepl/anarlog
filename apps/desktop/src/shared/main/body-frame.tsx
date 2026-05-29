import { MainChatPanels } from "./chat-panels";
import {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
} from "./session-status-banner";

export function MainShellBodyFrame({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <SessionStatusBannerProvider>
      <MainChatPanels>{children}</MainChatPanels>
      <MainSessionStatusBannerHost />
    </SessionStatusBannerProvider>
  );
}
