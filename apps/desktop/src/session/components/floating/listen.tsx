import { ListenActionButton } from "../listen-action";

import { useListenButtonState } from "~/session/components/shared";
import type { Tab } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function ListenButton({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const { shouldRender } = useListenButtonState(tab.id);
  const loading = useListener(
    (state) => state.live.loading && state.live.sessionId === tab.id,
  );

  if (loading) {
    return <ListenActionButton sessionId={tab.id} />;
  }

  if (!shouldRender) {
    return null;
  }

  return <ListenActionButton sessionId={tab.id} />;
}
