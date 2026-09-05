import { MainChatPanels } from "./chat-panels";

export function MainShellBodyFrame({
  children,
}: {
  children: React.ReactNode;
}) {
  return <MainChatPanels>{children}</MainChatPanels>;
}
