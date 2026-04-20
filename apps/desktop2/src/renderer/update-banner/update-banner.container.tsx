import { useState } from "react";

import { hypr } from "~/bridge";
import { UpdateBannerView } from "~/update-banner/update-banner.view";
import { useUpdate } from "~/update-banner/use-update";

export function UpdateBannerContainer() {
  const { version, progress, status } = useUpdate();
  const [dismissed, setDismissed] = useState(false);
  const [installing, setInstalling] = useState(false);

  if (
    !version ||
    dismissed ||
    status === "idle" ||
    status === "checking" ||
    status === "not-available" ||
    status === "error"
  ) {
    return null;
  }

  return (
    <UpdateBannerView
      version={version}
      progress={progress}
      installing={installing}
      onInstall={async () => {
        setInstalling(true);
        try {
          await hypr.updater.install();
        } finally {
          // quitAndInstall closes the app before this resolves, but if the
          // install is rejected we still want the button re-enabled.
          setInstalling(false);
        }
      }}
      onDismiss={() => setDismissed(true)}
    />
  );
}
