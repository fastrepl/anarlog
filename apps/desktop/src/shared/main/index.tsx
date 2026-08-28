import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";

import { colors } from "@anlg/design-system/tokens.stylex";
import {
  ResizablePanel,
  ResizablePanelGroup,
} from "@anlg/ui/components/ui/resizable";

import { mainSurface } from "./surface.stylex";

export { MainShellBodyFrame } from "./body-frame";
export { MainChatPanels } from "./chat-panels";
export { useMainContentCenterOffset } from "./content-offset";
export {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
  useSessionStatusBanner,
} from "./session-status-banner";
export { MainShellScaffold, type MainSurfaceChrome } from "./shell-scaffold";

export function StandardContentWrapper({
  children,
  floatingButton,
  noBorder = false,
}: {
  children: React.ReactNode;
  floatingButton?: React.ReactNode;
  noBorder?: boolean;
}) {
  return (
    <div {...stylex.props(styles.root)}>
      <ResizablePanelGroup direction="vertical" sx={styles.panelGroup}>
        <ResizablePanel defaultSize={100} minSize={35} sx={styles.panel}>
          <MainPanel fill floatingButton={floatingButton} noBorder={noBorder}>
            {children}
          </MainPanel>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

function MainPanel({
  children,
  fill,
  floatingButton,
  noBorder,
}: {
  children: React.ReactNode;
  fill: boolean;
  floatingButton?: React.ReactNode;
  noBorder: boolean;
}) {
  const isMacos = platform() === "macos";

  return (
    <div {...stylex.props(styles.mainPanel, fill && styles.fill)}>
      <div
        data-chat-floating-anchor
        {...stylex.props(
          styles.floatingAnchor,
          isMacos && styles.floatingAnchorMacos,
          !noBorder && styles.floatingAnchorBorder,
        )}
      >
        {children}
        {floatingButton}
      </div>
    </div>
  );
}

const styles = stylex.create({
  fill: {
    height: "100%",
  },
  floatingAnchor: {
    backgroundColor: colors.card,
    containerType: "inline-size",
    display: "flex",
    flex: "1",
    flexDirection: "column",
    minHeight: 0,
    overflow: "hidden",
    position: "relative",
  },
  floatingAnchorBorder: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: mainSurface.borderBottomWidth,
    borderLeftColor: colors.border,
    borderLeftStyle: "solid",
    borderLeftWidth: mainSurface.borderLeftWidth,
    borderRightColor: colors.border,
    borderRightStyle: "solid",
    borderRightWidth: mainSurface.borderRightWidth,
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: mainSurface.borderTopWidth,
  },
  floatingAnchorMacos: {
    borderBottomLeftRadius: mainSurface.radiusBottomLeft,
    borderBottomRightRadius: mainSurface.radiusBottomRight,
    borderTopLeftRadius: mainSurface.radiusTopLeft,
    borderTopRightRadius: mainSurface.radiusTopRight,
  },
  mainPanel: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    minHeight: 0,
    position: "relative",
  },
  panel: {
    minHeight: 0,
  },
  panelGroup: {
    flex: "1",
    minHeight: 0,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
});

export { styles as mainStyles };
