import { commands as windowsCommands } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsDontUseThis() {
  const handleOpenControlWindow = async () => {
    await windowsCommands.windowShow({ type: "control" });
  };

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title="General" />
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 className="mb-1 text-sm font-medium">Control Overlay</h3>
          <p className="text-xs text-neutral-600">
            Floating window for quick access to recording controls.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={handleOpenControlWindow}>
          Open
        </Button>
      </div>
    </div>
  );
}
