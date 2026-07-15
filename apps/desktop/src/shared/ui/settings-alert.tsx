import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function SettingsAlertToast({
  id,
  description,
  variant = "default",
}: {
  id: string;
  description?: string;
  variant?: "default" | "error" | "warning";
}) {
  if (!description) {
    return null;
  }

  return (
    <SettingsAlertToastLifecycle
      key={`${id}:${description}`}
      id={id}
      description={description}
      variant={variant}
    />
  );
}

function SettingsAlertToastLifecycle({
  id,
  description,
  variant,
}: {
  id: string;
  description: string;
  variant: "default" | "error" | "warning";
}) {
  useMountEffect(() => {
    const options = { id, duration: Infinity };

    if (variant === "error") {
      sonnerToast.error(description, options);
    } else if (variant === "warning") {
      sonnerToast.warning(description, options);
    } else {
      sonnerToast.message(description, options);
    }

    return () => {
      sonnerToast.dismiss(id);
    };
  });

  return null;
}
