import type { MouseEvent } from "react";

import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function SettingsAlertToast({
  id,
  description,
  variant = "default",
  dismissible,
  action,
}: {
  id: string;
  description?: string;
  variant?: "default" | "error" | "warning";
  dismissible?: boolean;
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
      key={`${id}:${description}:${dismissible ?? "default"}:${action?.label ?? ""}`}
      id={id}
      description={description}
      variant={variant}
      dismissible={dismissible}
      action={action}
    />
  );
}

function SettingsAlertToastLifecycle({
  id,
  description,
  variant,
  dismissible,
  action,
}: {
  id: string;
  description: string;
  variant: "default" | "error" | "warning";
  dismissible?: boolean;
  action?: {
    label: string;
    onClick: () => void | Promise<void>;
  };
}) {
  useMountEffect(() => {
    const options = {
      id,
      duration: Infinity,
      ...(dismissible === undefined
        ? {}
        : {
            dismissible,
            ...(dismissible ? {} : { closeButton: false }),
          }),
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
