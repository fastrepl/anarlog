import { useSessionTabLifecycle } from "~/session/hooks/sessions";

export function useMain2Lifecycle() {
  useSessionTabLifecycle({
    onEmpty: null,
    onZeroTabs: null,
  });
}
