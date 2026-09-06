import { HStack, Image, Label, Menu, Picker, Text } from "@expo/ui/swift-ui";
import {
  disabled,
  labelsHidden,
  layoutPriority,
  pickerStyle,
  tag,
} from "@expo/ui/swift-ui/modifiers";

import { ProviderIcon } from "./provider-icon";
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
    <Menu
      label={
        <HStack spacing={8}>
          <ProviderIcon provider={selectedValue} size={20} />
          <Text>
            {providersFor(kind).find(({ id }) => id === selectedValue)?.name}
          </Text>
          <Image systemName="chevron.up.chevron.down" size={12} />
        </HStack>
      }
      modifiers={[layoutPriority(1), disabled(!enabled)]}
      testID="active-provider"
    >
      <Picker
        label="Provider"
        selection={selectedValue}
        onSelectionChange={onValueChange}
        modifiers={[pickerStyle("inline"), labelsHidden()]}
      >
        {providersFor(kind).map((provider) => (
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
