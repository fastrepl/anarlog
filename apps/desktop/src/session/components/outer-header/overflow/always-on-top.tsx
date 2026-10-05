import { Trans } from "@lingui/react/macro";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useState } from "react";

import { Check, PushPin } from "@anlg/ui/components/icons";
import { DropdownMenuItem } from "@anlg/ui/components/ui/dropdown-menu";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

let requestedAlwaysOnTop: boolean | null = null;

export function AlwaysOnTop() {
  const [enabled, setEnabled] = useState<boolean | null>(requestedAlwaysOnTop);

  useMountEffect(() => {
    if (requestedAlwaysOnTop === null) {
      getCurrentWindow()
        .isAlwaysOnTop()
        .then(setEnabled)
        .catch((error) => {
          console.error("Failed to read always-on-top state", error);
        });
    }
  });

  return (
    <DropdownMenuItem
      onClick={(e) => {
        e.preventDefault();
        const previous = enabled ?? false;
        const next = !previous;
        setEnabled(next);
        requestedAlwaysOnTop = next;
        getCurrentWindow()
          .setAlwaysOnTop(next)
          .catch((error) => {
            console.error("Failed to set always-on-top state", error);
            setEnabled(previous);
            requestedAlwaysOnTop = previous;
          });
      }}
      disabled={enabled == null}
      className="cursor-pointer"
    >
      <PushPin />
      <span>
        <Trans>Always on Top</Trans>
      </span>
      {enabled && <Check className="ml-auto" />}
    </DropdownMenuItem>
  );
}
