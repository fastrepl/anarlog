import {
  validateProviderApiKey,
  validateProviderConfig,
  type ProviderConfig,
  type ProviderKind,
} from "./providers-model";

export function createProviderAutosave(
  kind: ProviderKind,
  save: (draft: { config: ProviderConfig; apiKey: string }) => void,
) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: { config: ProviderConfig; apiKey: string } | undefined;

  function cancel() {
    clearTimeout(timer);
    timer = undefined;
    pending = undefined;
  }

  function flush() {
    const draft = pending;
    cancel();
    if (draft) save(draft);
  }

  return {
    schedule(config: ProviderConfig, apiKey: string, hasSavedKey: boolean) {
      cancel();
      try {
        const normalized = validateProviderConfig(kind, config);
        const key = apiKey.trim();
        if (key || !hasSavedKey) validateProviderApiKey(key);
        pending = { config: normalized, apiKey: key };
      } catch {
        return;
      }
      timer = setTimeout(flush, 500);
    },
    flush,
    cancel,
  };
}
