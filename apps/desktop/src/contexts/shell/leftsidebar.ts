import { useCallback, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { useConfigValue } from "~/shared/config";

const forceDevtoolsPanel =
  import.meta.env.DEV && import.meta.env.MODE !== "test";

export function useLeftSidebar() {
  const [expanded, setExpanded] = useState(true);
  const [locked, setLocked] = useState(false);
  const [showDevtool, setShowDevtool] = useState(false);
  const devtoolsControlPanelEnabled = useConfigValue(
    "devtools_control_panel_enabled",
  );

  const toggleExpanded = useCallback(() => {
    if (locked) return;
    setExpanded((prev) => !prev);
  }, [locked]);

  const toggleDevtool = useCallback(() => {
    setShowDevtool((prev) => !prev);
  }, []);

  useHotkeys(
    "mod+\\",
    toggleExpanded,
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [toggleExpanded],
  );

  return {
    expanded,
    setExpanded,
    locked,
    setLocked,
    toggleExpanded,
    showDevtool:
      forceDevtoolsPanel || devtoolsControlPanelEnabled || showDevtool,
    setShowDevtool,
    toggleDevtool,
  };
}
