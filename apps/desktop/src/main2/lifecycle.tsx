import { useMainIndexes, useMainStore } from "~/session/hooks/storage";
import { useDesktopTabLifecycle } from "~/shared/desktop-tab-lifecycle";

export function useMain2Lifecycle() {
  const store = useMainStore();
  const indexes = useMainIndexes();

  useDesktopTabLifecycle({
    store,
    indexes,
    onEmpty: null,
    onZeroTabs: null,
  });
}
