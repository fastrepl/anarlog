import { t } from "@lingui/core/macro";
import {
  type ErrorRouteComponent,
  NotFoundRouteComponent,
  useNavigate,
} from "@tanstack/react-router";
import { relaunch } from "@tauri-apps/plugin-process";
import { motion } from "motion/react";

import {
  ArrowClockwise,
  House,
  MagnifyingGlass,
  Warning,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { captureOperationalError } from "~/error-reporting";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

const routeErrorKeys = new WeakMap<object, number>();
let nextRouteErrorKey = 0;

function getRouteErrorKey(error: unknown) {
  if (typeof error !== "object" || error === null) {
    return String(error);
  }

  const existing = routeErrorKeys.get(error);
  if (existing !== undefined) {
    return existing;
  }

  nextRouteErrorKey += 1;
  routeErrorKeys.set(error, nextRouteErrorKey);
  return nextRouteErrorKey;
}

const ReportedErrorComponent = ({ error }: { error: Error }) => {
  useMountEffect(() => {
    captureOperationalError(error, {
      operation: "route_render",
    });
  });

  const handleRestart = async () => {
    try {
      await relaunch();
    } catch (err) {
      captureOperationalError(err, {
        operation: "app_restart",
      });
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div
        data-tauri-drag-region
        className="fixed inset-x-0 top-0 z-50 h-10 bg-transparent"
      />

      <div className="flex h-full min-h-[300px] items-center justify-center p-6">
        <motion.div
          className="w-full max-w-sm"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        >
          <div className="border-border bg-card rounded-xl border p-6 shadow-xs">
            <div className="flex flex-col items-center gap-4 text-center">
              <motion.div
                className="flex h-12 w-12 items-center justify-center rounded-full bg-red-50"
                initial={{ scale: 0.8 }}
                animate={{ scale: 1 }}
                transition={{
                  delay: 0.1,
                  type: "spring",
                  stiffness: 200,
                }}
              >
                <Warning className="h-6 w-6 text-red-500" />
              </motion.div>

              <div className="flex flex-col gap-1.5">
                <h2 className="text-foreground text-base font-semibold">
                  {t`Something went wrong`}
                </h2>
                <p className="text-muted-foreground max-w-[260px] text-sm leading-relaxed">
                  {error.message || t`An unexpected error occurred.`}
                </p>
              </div>

              <div className="pt-2">
                <Button size="sm" onClick={handleRestart}>
                  <ArrowClockwise className="mr-1.5 h-3.5 w-3.5" />
                  {t`Restart App`}
                </Button>
              </div>
            </div>
          </div>
        </motion.div>
      </div>
    </div>
  );
};

export const ErrorComponent: ErrorRouteComponent = ({ error }) => (
  <ReportedErrorComponent key={getRouteErrorKey(error)} error={error} />
);

export const NotFoundComponent: NotFoundRouteComponent = () => {
  const navigate = useNavigate();

  return (
    <div className="flex h-full flex-col">
      <div
        data-tauri-drag-region
        className="fixed inset-x-0 top-0 z-50 h-10 bg-transparent"
      />

      <div className="flex h-full min-h-[300px] items-center justify-center p-6">
        <motion.div
          className="w-full max-w-sm"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        >
          <div className="border-border bg-card rounded-xl border p-6 shadow-xs">
            <div className="flex flex-col items-center gap-4 text-center">
              <motion.div
                className="bg-muted flex h-12 w-12 items-center justify-center rounded-full"
                initial={{ scale: 0.8 }}
                animate={{ scale: 1 }}
                transition={{
                  delay: 0.1,
                  type: "spring",
                  stiffness: 200,
                }}
              >
                <MagnifyingGlass className="text-muted-foreground h-6 w-6" />
              </motion.div>

              <div className="flex flex-col gap-1.5">
                <motion.span
                  className="text-muted-foreground/70 block text-4xl font-bold"
                  initial={{ scale: 0.9, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
                  transition={{
                    delay: 0.15,
                    type: "spring",
                    stiffness: 200,
                  }}
                >
                  404
                </motion.span>
                <h2 className="text-foreground text-base font-semibold">
                  {t`Page not found`}
                </h2>
                <p className="text-muted-foreground text-sm">
                  {t`The page you're looking for doesn't exist.`}
                </p>
              </div>

              <div className="pt-2">
                <Button size="sm" onClick={() => navigate({ to: "/app" })}>
                  <House className="mr-1.5 h-3.5 w-3.5" />
                  {t`Go to Home`}
                </Button>
              </div>
            </div>
          </div>
        </motion.div>
      </div>
    </div>
  );
};
