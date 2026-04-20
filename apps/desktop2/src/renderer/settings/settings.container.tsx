import { CommandLineContainer } from "~/settings/command-line";
import { SettingsView } from "~/settings/settings.view";

export function SettingsContainer() {
  return (
    <SettingsView>
      <CommandLineContainer />
    </SettingsView>
  );
}
