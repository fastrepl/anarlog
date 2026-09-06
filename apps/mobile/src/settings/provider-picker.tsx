import { Picker } from "@expo/ui";

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
  return (
    <Picker
      selectedValue={selectedValue}
      enabled={enabled}
      onValueChange={onValueChange}
      testID="active-provider"
    >
      {!selectedValue && <Picker.Item value="" label="Select provider" />}
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
