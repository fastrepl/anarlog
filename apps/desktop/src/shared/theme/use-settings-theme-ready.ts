import { Status } from "tinybase/persisters";

import * as settings from "~/store/tinybase/store/settings";

export function useSettingsThemeReady(): boolean {
  const persister = settings.UI.usePersister(settings.STORE_ID);
  const persisterStatus = settings.UI.usePersisterStatus(settings.STORE_ID);

  return persister != null && persisterStatus !== Status.Loading;
}
