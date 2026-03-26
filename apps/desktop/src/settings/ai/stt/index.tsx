import { ConfigureProviders } from "./configure";
import { SttSettingsProvider } from "./context";
import { SelectProviderAndModel } from "./select";

export function STT() {
  return (
    <SttSettingsProvider>
      <div className="mt-4 flex flex-col gap-6">
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </SttSettingsProvider>
  );
}
