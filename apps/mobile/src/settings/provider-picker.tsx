import { Picker } from "@expo/ui";

import { providersFor, type ProviderKind } from "./providers-model";

export function ProviderPicker({
  kind,
  selectedValue,
  enabled,
  onValueChange,
}: {
  kind: ProviderKind;
  selectedValue: string;
  enabled: boolean;
  onValueChange: (provider: string) => void;
}) {
  return (
    <Picker
      selectedValue={selectedValue}
      enabled={enabled}
      onValueChange={onValueChange}
      testID="active-provider"
    >
      {providersFor(kind).map((provider) => (
        <Picker.Item
          key={provider.id}
          value={provider.id}
          label={provider.name}
        />
      ))}
    </Picker>
  );
}
