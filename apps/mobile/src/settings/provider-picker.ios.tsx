import { HStack, Image, Label, Menu, Picker, Text } from "@expo/ui/swift-ui";
import {
  disabled,
  labelsHidden,
  layoutPriority,
  pickerStyle,
  tag,
} from "@expo/ui/swift-ui/modifiers";

import { ProviderIcon } from "./provider-icon";

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
  const selected = providers.find(({ id }) => id === selectedValue);
  return (
    <Menu
      label={
        <HStack spacing={8}>
          {selected && <ProviderIcon provider={selected.id} size={20} />}
          <Text>{selected?.name ?? "Select provider"}</Text>
          <Image systemName="chevron.up.chevron.down" size={12} />
        </HStack>
      }
      modifiers={[layoutPriority(1), disabled(!enabled)]}
      testID="active-provider"
    >
      <Picker
        label="Provider"
        selection={selected?.id}
        onSelectionChange={onValueChange}
        modifiers={[pickerStyle("inline"), labelsHidden()]}
      >
        {providers.map((provider) => (
          <Label
            key={provider.id}
            title={provider.name}
            icon={<ProviderIcon provider={provider.id} />}
            modifiers={[tag(provider.id)]}
          />
        ))}
      </Picker>
    </Menu>
  );
}
