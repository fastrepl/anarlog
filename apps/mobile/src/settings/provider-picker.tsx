import { Picker } from "@expo/ui";

import { translate, useLanguage } from "@/i18n";

export function ProviderPicker({
  providers,
  selectedValue,
  enabled,
  onValueChange,
}: {
  providers: readonly { id: string; name: string }[];
  selectedValue: string;
  enabled: boolean;
  onValueChange: (provider: string) => void;
}) {
  const { language } = useLanguage();
  return (
    <Picker
      selectedValue={selectedValue}
      enabled={enabled}
      onValueChange={onValueChange}
      testID="active-provider"
    >
      {!selectedValue && (
        <Picker.Item value="" label={translate("Select provider", language)} />
      )}
      {providers.map((provider) => (
        <Picker.Item
          key={provider.id}
          value={provider.id}
          label={provider.name}
        />
      ))}
    </Picker>
  );
}
