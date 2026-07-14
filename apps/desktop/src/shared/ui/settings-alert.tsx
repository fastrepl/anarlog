import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  showTransientToast,
  useTransientToast,
} from "~/sidebar/toast/transient";

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
    showTransientToast(
      {
        id,
        description,
        anchor: "main-content-panel",
        dismissible: false,
        variant,
      },
      { durationMs: null },
    );

    return () => {
      const { toast, clearToast } = useTransientToast.getState();
      if (toast?.id === id) {
        clearToast(toast.key);
      }
    };
  });

  return null;
}
