import { useLingui } from "@lingui/react/macro";
import {
  type ReactNode,
  type SyntheticEvent,
  useCallback,
  useRef,
} from "react";

import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { useBillingAccess } from "~/auth/billing-context";

export type GatedPlan = "pro" | "team" | "enterprise";

export function useNotifyPlanRequired() {
  const { t } = useLingui();
  const { upgradeToPro } = useBillingAccess();

  return useCallback(
    (plan: GatedPlan) => {
      const message =
        plan === "pro"
          ? t`This requires Anarlog Pro`
          : plan === "team"
            ? t`This requires Anarlog Team`
            : t`This requires Anarlog Enterprise`;

      sonnerToast.warning(message, {
        ...(plan === "pro"
          ? {
              action: {
                label: t`Upgrade`,
                onClick: () => {
                  upgradeToPro();
                },
              },
            }
          : {}),
      });
    },
    [t, upgradeToPro],
  );
}

export function PlanGate({
  plan,
  allowed,
  children,
  className,
}: {
  plan: GatedPlan;
  allowed: boolean;
  children: ReactNode;
  className?: string;
}) {
  const notify = useNotifyPlanRequired();
  const lastNotifyAtRef = useRef(0);

  const block = (event: SyntheticEvent) => {
    event.preventDefault();
    event.stopPropagation();
    const now = Date.now();
    if (now - lastNotifyAtRef.current < 400) {
      return;
    }
    lastNotifyAtRef.current = now;
    notify(plan);
  };

  if (allowed) {
    return children;
  }

  return (
    <div
      className={cn(["relative cursor-not-allowed opacity-60", className])}
      onPointerDownCapture={block}
      onClickCapture={block}
      onKeyDownCapture={(event) => {
        if (event.key === "Tab") {
          return;
        }
        block(event);
      }}
    >
      {children}
    </div>
  );
}
