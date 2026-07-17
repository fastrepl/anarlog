import { getStoredSettingValues, setSettingValues } from "~/settings/queries";
import type { SettingValues } from "~/settings/schema";

export async function configurePaidSettings(): Promise<void> {
  const { values } = await getStoredSettingValues();
  const updates: SettingValues = {};

  if (!values.current_stt_provider) {
    updates.current_stt_provider = "hyprnote";
    updates.current_stt_model = "cloud";
  }

  if (!values.current_llm_provider) {
    updates.current_llm_provider = "hyprnote";
    updates.current_llm_model = "Auto";
  }

  await setSettingValues(updates);
}
