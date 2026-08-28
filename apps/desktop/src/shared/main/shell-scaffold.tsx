import * as stylex from "@stylexjs/stylex";
import { Fragment } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";

import { mainSurface } from "./surface.stylex";

import { SyncProvider } from "~/calendar/components/context";
import { useTabs } from "~/store/zustand/tabs";

export type MainSurfaceChrome = "default" | "top" | "top-borderless" | "left";

export function MainShellScaffold({
  children,
  edgeToEdge = false,
  mainSurfaceChrome,
}: {
  children: React.ReactNode;
  edgeToEdge?: boolean;
  mainSurfaceChrome?: MainSurfaceChrome;
}) {
  const currentTab = useTabs((state) => state.currentTab);
  const isCalendarMode = currentTab?.type === "calendar";
  const SyncWrapper = isCalendarMode ? SyncProvider : Fragment;
  const resolvedMainSurfaceChrome =
    mainSurfaceChrome ?? (edgeToEdge ? "top" : "default");
  const hasTopMainSurfaceChrome =
    resolvedMainSurfaceChrome === "top" ||
    resolvedMainSurfaceChrome === "top-borderless";

  return (
    <SyncWrapper>
      <div
        {...stylex.props(
          styles.root,
          !hasTopMainSurfaceChrome && styles.leftPadding,
          hasTopMainSurfaceChrome &&
            (resolvedMainSurfaceChrome === "top"
              ? styles.topChrome
              : styles.topBorderlessChrome),
          resolvedMainSurfaceChrome === "left" && styles.leftChrome,
        )}
        data-testid="main-app-shell"
      >
        {children}
      </div>
    </SyncWrapper>
  );
}

const styles = stylex.create({
  leftChrome: {
    [mainSurface.borderBottomWidth]: "0px",
    [mainSurface.borderLeftWidth]: "1px",
    [mainSurface.borderRightWidth]: "0px",
    [mainSurface.borderTopWidth]: "0px",
    [mainSurface.radiusBottomRight]: "0px",
    [mainSurface.radiusTopRight]: "0px",
  },
  leftPadding: {
    paddingLeft: "0.25rem",
  },
  root: {
    backgroundColor: colors.background,
    display: "flex",
    gap: "0.25rem",
    height: "100%",
    overflow: "hidden",
  },
  topBorderlessChrome: {
    [mainSurface.borderBottomWidth]: "0px",
    [mainSurface.borderLeftWidth]: "0px",
    [mainSurface.borderRightWidth]: "0px",
    [mainSurface.borderTopWidth]: "0px",
    [mainSurface.radiusBottomLeft]: "0px",
    [mainSurface.radiusBottomRight]: "0px",
  },
  topChrome: {
    [mainSurface.borderBottomWidth]: "0px",
    [mainSurface.borderLeftWidth]: "0px",
    [mainSurface.borderRightWidth]: "0px",
    [mainSurface.borderTopWidth]: "1px",
    [mainSurface.radiusBottomLeft]: "0px",
    [mainSurface.radiusBottomRight]: "0px",
  },
});

export { styles as mainShellScaffoldStyles };
