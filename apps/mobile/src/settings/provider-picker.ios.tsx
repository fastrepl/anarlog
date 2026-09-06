import { Label, Picker } from "@expo/ui/swift-ui";
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
    <Picker
      selection={selectedValue}
      onSelectionChange={onValueChange}
      modifiers={[
        pickerStyle("menu"),
        labelsHidden(),
        layoutPriority(1),
        disabled(!enabled),
      ]}
      testID="active-provider"
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
  );
}
