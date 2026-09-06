import { Icon, Row, Text } from "@expo/ui";
import {
  ExposedDropdownMenu,
  ExposedDropdownMenuBox,
  DropdownMenuItem,
} from "@expo/ui/jetpack-compose";
import { menuAnchor } from "@expo/ui/jetpack-compose/modifiers";
import { useState } from "react";

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
  const [expanded, setExpanded] = useState(false);
  const selected = providers.find(({ id }) => id === selectedValue);
  return (
    <ExposedDropdownMenuBox
      expanded={expanded}
      onExpandedChange={enabled ? setExpanded : undefined}
    >
      <Row
        modifiers={[menuAnchor()]}
        spacing={8}
        alignment="center"
        testID="active-provider"
      >
        {selected && <ProviderIcon provider={selected.id} />}
        <Text>{selected?.name ?? "Select provider"}</Text>
        <Icon
          name={Icon.select({
            ios: "chevron.down",
            android: import("@expo/material-symbols/keyboard_arrow_down.xml"),
          })}
          size={16}
        />
      </Row>
      <ExposedDropdownMenu
        expanded={expanded}
        onDismissRequest={() => setExpanded(false)}
      >
        {providers.map((provider) => (
          <DropdownMenuItem
            key={provider.id}
            enabled={enabled}
            onClick={() => {
              setExpanded(false);
              onValueChange(provider.id);
            }}
          >
            <DropdownMenuItem.LeadingIcon>
              <ProviderIcon provider={provider.id} />
            </DropdownMenuItem.LeadingIcon>
            <DropdownMenuItem.Text>
              <Text>{provider.name}</Text>
            </DropdownMenuItem.Text>
            {selectedValue === provider.id && (
              <DropdownMenuItem.TrailingIcon>
                <Icon
                  name={Icon.select({
                    ios: "checkmark",
                    android: import("@expo/material-symbols/check.xml"),
                  })}
                  size={18}
                />
              </DropdownMenuItem.TrailingIcon>
            )}
          </DropdownMenuItem>
        ))}
      </ExposedDropdownMenu>
    </ExposedDropdownMenuBox>
  );
}
