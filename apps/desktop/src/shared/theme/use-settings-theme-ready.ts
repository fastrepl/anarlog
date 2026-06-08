import * as settings from "~/store/tinybase/store/settings";

const PERSISTER_STATUS_LOADING = 1;

export function useSettingsThemeReady(): boolean {
  const persister = settings.UI.usePersister(settings.STORE_ID);
  const persisterStatus = settings.UI.usePersisterStatus(settings.STORE_ID);

  return persister != null && persisterStatus !== PERSISTER_STATUS_LOADING;
}
