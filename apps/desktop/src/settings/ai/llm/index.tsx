import { ConfigureProviders } from "./configure";
import { LlmSettingsProvider } from "./context";
import { SelectProviderAndModel } from "./select";

export function LLM() {
  return (
    <LlmSettingsProvider>
      <div className="mt-4 flex flex-col gap-6">
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </LlmSettingsProvider>
  );
}
