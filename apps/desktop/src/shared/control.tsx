import { t } from "@lingui/core/macro";
import {
  ArrowClockwise,
  House,
  MagnifyingGlass,
  Warning,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import {
  type ErrorRouteComponent,
  NotFoundRouteComponent,
  useNavigate,
} from "@tanstack/react-router";
import { relaunch } from "@tauri-apps/plugin-process";
import { motion } from "motion/react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
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
    <div {...stylex.props(styles.root)}>
      <div data-tauri-drag-region {...stylex.props(styles.dragRegion)} />

      <div {...stylex.props(styles.center)}>
        <motion.div
          {...stylex.props(styles.motion)}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        >
          <div {...stylex.props(styles.card)}>
            <div {...stylex.props(styles.content)}>
              <motion.div
                {...stylex.props(styles.iconCircle, styles.errorIconCircle)}
                initial={{ scale: 0.8 }}
                animate={{ scale: 1 }}
                transition={{
                  delay: 0.1,
                  type: "spring",
                  stiffness: 200,
                }}
              >
                <Warning {...stylex.props(styles.icon, styles.errorIcon)} />
              </motion.div>

              <div {...stylex.props(styles.copy)}>
                <h2 {...stylex.props(styles.title)}>
                  {t`Something went wrong`}
                </h2>
                <p {...stylex.props(styles.errorDescription)}>
                  {error.message || t`An unexpected error occurred.`}
                </p>
              </div>

              <div {...stylex.props(styles.action)}>
                <Button size="sm" onClick={handleRestart}>
                  <ArrowClockwise {...stylex.props(styles.buttonIcon)} />
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
    <div {...stylex.props(styles.root)}>
      <div data-tauri-drag-region {...stylex.props(styles.dragRegion)} />

      <div {...stylex.props(styles.center)}>
        <motion.div
          {...stylex.props(styles.motion)}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        >
          <div {...stylex.props(styles.card)}>
            <div {...stylex.props(styles.content)}>
              <motion.div
                {...stylex.props(styles.iconCircle, styles.notFoundIconCircle)}
                initial={{ scale: 0.8 }}
                animate={{ scale: 1 }}
                transition={{
                  delay: 0.1,
                  type: "spring",
                  stiffness: 200,
                }}
              >
                <MagnifyingGlass
                  {...stylex.props(styles.icon, styles.notFoundIcon)}
                />
              </motion.div>

              <div {...stylex.props(styles.copy)}>
                <motion.span
                  {...stylex.props(styles.statusCode)}
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
                <h2 {...stylex.props(styles.title)}>{t`Page not found`}</h2>
                <p {...stylex.props(styles.description)}>
                  {t`The page you're looking for doesn't exist.`}
                </p>
              </div>

              <div {...stylex.props(styles.action)}>
                <Button size="sm" onClick={() => navigate({ to: "/app" })}>
                  <House {...stylex.props(styles.buttonIcon)} />
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

const styles = stylex.create({
  action: {
    paddingTop: "0.5rem",
  },
  buttonIcon: {
    height: "0.875rem",
    marginRight: "0.375rem",
    width: "0.875rem",
  },
  card: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    padding: "1.5rem",
  },
  center: {
    alignItems: "center",
    display: "flex",
    height: "100%",
    justifyContent: "center",
    minHeight: "300px",
    padding: "1.5rem",
  },
  content: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    textAlign: "center",
  },
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.375rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  dragRegion: {
    backgroundColor: "transparent",
    height: "2.5rem",
    left: 0,
    position: "fixed",
    right: 0,
    top: 0,
    zIndex: 50,
  },
  errorDescription: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
    maxWidth: "260px",
  },
  errorIcon: {
    color: "rgb(239 68 68)",
  },
  errorIconCircle: {
    backgroundColor: "rgb(254 242 242)",
  },
  icon: {
    height: "1.5rem",
    width: "1.5rem",
  },
  iconCircle: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    height: "3rem",
    justifyContent: "center",
    width: "3rem",
  },
  motion: {
    maxWidth: "24rem",
    width: "100%",
  },
  notFoundIcon: {
    color: colors.mutedForeground,
  },
  notFoundIconCircle: {
    backgroundColor: colors.muted,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  statusCode: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    display: "block",
    fontSize: "2.25rem",
    fontWeight: 700,
    lineHeight: "2.5rem",
  },
  title: {
    color: colors.foreground,
    fontSize: "1rem",
    fontWeight: 600,
  },
});
