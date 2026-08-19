import type { MouseEvent } from "react";

import { sonnerToast, TOAST_DURATIONS } from "@anlg/ui/components/ui/toast";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function SettingsAlertToast({
  id,
  description,
  variant = "default",
  lifecycle,
  action,
}: {
  id: string;
  description?: string;
  variant?: "default" | "error" | "warning";
  lifecycle: "condition-bound" | "persistent";
  action?: {
    label: string;
    onClick: () => void | Promise<void>;
  };
}) {
  if (!description) {
    return null;
  }

  return (
    <SettingsAlertToastLifecycle
      key={`${id}:${description}:${lifecycle}:${action?.label ?? ""}`}
      id={id}
      description={description}
      variant={variant}
      lifecycle={lifecycle}
      action={action}
    />
  );
}

function SettingsAlertToastLifecycle({
  id,
  description,
  variant,
  lifecycle,
  action,
}: {
  id: string;
  description: string;
  variant: "default" | "error" | "warning";
  lifecycle: "condition-bound" | "persistent";
  action?: {
    label: string;
    onClick: () => void | Promise<void>;
  };
}) {
  useMountEffect(() => {
    const dismissible = lifecycle === "persistent";
    const options = {
      id,
      duration: variant === "error" ? TOAST_DURATIONS.error : Infinity,
      dismissible,
      closeButton: dismissible,
      ...(action
        ? {
            action: {
              label: action.label,
              onClick: (event: MouseEvent<HTMLButtonElement>) => {
                event.preventDefault();
                void action.onClick();
              },
            },
          }
        : {}),
    };

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
