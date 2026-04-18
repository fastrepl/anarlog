import { useSessionTabLifecycle } from "~/session/hooks/storage";

export function useMain2Lifecycle() {
  useSessionTabLifecycle({
    onEmpty: null,
    onZeroTabs: null,
  });
}
