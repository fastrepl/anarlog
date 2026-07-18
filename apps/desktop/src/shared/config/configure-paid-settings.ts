import { PROVIDERS } from "~/settings/ai/llm/shared";
import { getProviderSelectionBlockers } from "~/settings/ai/shared/eligibility";
import { getStoredAiProvider } from "~/settings/providers";
import { getStoredSettingValues, setSettingValues } from "~/settings/queries";
import type { SettingValues } from "~/settings/schema";

export async function configurePaidSettings(): Promise<void> {
  const { values } = await getStoredSettingValues();
  const updates: SettingValues = {};

  if (!values.current_stt_provider) {
    updates.current_stt_provider = "hyprnote";
    updates.current_stt_model = "cloud";
  }

  if (await shouldUseHostedLlm(values)) {
    updates.current_llm_provider = "hyprnote";
    updates.current_llm_model = "Auto";
  }

  await setSettingValues(updates);
}

async function shouldUseHostedLlm(values: SettingValues): Promise<boolean> {
  const providerId = values.current_llm_provider;
  if (!providerId || !values.current_llm_model) return true;

  const provider = PROVIDERS.find((candidate) => candidate.id === providerId);
  if (!provider) return true;

  const defaultConfig = {
    base_url: provider.baseUrl || "",
    api_key: "",
  };
  if (
    getProviderSelectionBlockers(provider.requirements, {
      isAuthenticated: true,
      isPaid: true,
      config: defaultConfig,
    }).length === 0
  ) {
    return false;
  }

  let config;
  try {
    config = await getStoredAiProvider("llm", providerId);
  } catch {
    return true;
  }

  return (
    getProviderSelectionBlockers(provider.requirements, {
      isAuthenticated: true,
      isPaid: true,
      config: {
        base_url: config?.base_url || defaultConfig.base_url,
        api_key: config?.api_key || "",
      },
    }).length > 0
  );
}
