import { useDesktopTabLifecycle } from "~/shared/desktop-tab-lifecycle";
import * as main from "~/store/tinybase/store/main";

export function useMain2Lifecycle() {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);

  useDesktopTabLifecycle({
    store,
    indexes,
    onEmpty: null,
    onZeroTabs: null,
  });
}
