import { Trans, useLingui } from "@lingui/react/macro";
import { X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { ChangelogContent } from "@anlg/changelog";
import { colors } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { useChangelogContent } from "./data";

import { useShell } from "~/contexts/shell";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useWindowControlsGutter } from "~/shared/hooks/useWindowControlsGutter";
import { StandardContentWrapper } from "~/shared/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export { getLatestVersion } from "./data";

export function TabContentChangelog({
  tab,
}: {
  tab: Extract<Tab, { type: "changelog" }>;
}) {
  const { current } = tab.state;
  const { chat, leftsidebar } = useShell();
  const close = useTabs((state) => state.close);
  const showSidebarTimelineHeaderGutter = !leftsidebar.expanded;
  const showExpandedSidebarTimelineHeader = leftsidebar.expanded;

  useMountEffect(() => {
    if (chat.mode !== "FloatingClosed") {
      chat.sendEvent({ type: "CLOSE" });
    }
  });

  const { content, loading } = useChangelogContent(current);

  return (
    <StandardContentWrapper>
      <div {...stylex.props(styles.root)}>
        <div data-tauri-drag-region {...stylex.props(styles.headerFrame)}>
          <ChangelogHeader
            version={current}
            showSidebarTimelineHeaderGutter={showSidebarTimelineHeaderGutter}
            showExpandedSidebarTimelineHeader={
              showExpandedSidebarTimelineHeader
            }
            onClose={() => close(tab)}
          />
        </div>

        <div {...stylex.props(styles.bodyFrame)}>
          <div {...mergeStyleXProps(styles.scroll, "scroll-fade-y")}>
            <ChangelogBody content={content} loading={loading} />
          </div>
        </div>
      </div>
    </StandardContentWrapper>
  );
}

function ExternalLink({
  href,
  children,
}: {
  href: string;
  children: React.ReactNode;
}) {
  return (
    <a
      {...stylex.props(styles.externalLink)}
      href={href}
      onClick={(e) => {
        e.preventDefault();
        void openerCommands.openUrl(href, null);
      }}
    >
      {children}
    </a>
  );
}

function ChangelogBody({
  content,
  loading,
}: {
  content: string | null;
  loading: boolean;
}) {
  if (loading) {
    return (
      <p {...stylex.props(styles.muted)}>
        <Trans>Loading...</Trans>
      </p>
    );
  }

  if (content) {
    return (
      <ChangelogContent
        content={content}
        components={{
          a: ({
            href,
            children,
          }: {
            href?: string;
            children?: React.ReactNode;
          }) =>
            href ? (
              <ExternalLink href={href}>{children}</ExternalLink>
            ) : (
              <>{children}</>
            ),
        }}
      />
    );
  }

  return (
    <p {...stylex.props(styles.muted)}>
      <Trans>No changelog available for this version.</Trans>
    </p>
  );
}

function ChangelogHeader({
  showExpandedSidebarTimelineHeader,
  showSidebarTimelineHeaderGutter,
  version,
  onClose,
}: {
  showExpandedSidebarTimelineHeader: boolean;
  showSidebarTimelineHeaderGutter: boolean;
  version: string;
  onClose: () => void;
}) {
  const { t } = useLingui();
  const showWindowControlsGutter = useWindowControlsGutter();
  return (
    <div
      data-tauri-drag-region
      {...stylex.props(
        styles.header,
        showSidebarTimelineHeaderGutter &&
          (showWindowControlsGutter
            ? styles.headerMacosGutter
            : styles.headerGutter),
      )}
    >
      <div
        data-tauri-drag-region
        {...stylex.props(
          styles.titleSlot,
          showExpandedSidebarTimelineHeader
            ? styles.titleSlotExpanded
            : showSidebarTimelineHeaderGutter
              ? [
                  styles.titleSlotGutter,
                  showWindowControlsGutter
                    ? styles.titleSlotMacos
                    : styles.titleSlotStandard,
                ]
              : styles.titleSlotCentered,
        )}
      >
        <h1
          {...stylex.props(
            styles.title,
            showExpandedSidebarTimelineHeader || showSidebarTimelineHeaderGutter
              ? styles.titleLeft
              : styles.titleCentered,
          )}
        >
          <Trans>What's new in {version}?</Trans>
        </h1>
      </div>

      <div {...stylex.props(styles.actions)}>
        <Button
          size="icon"
          variant="ghost"
          data-tauri-drag-region="false"
          sx={styles.close}
          aria-label={t`Close changelog`}
          title={t`Close`}
          onClick={onClose}
        >
          <X size={16} />
        </Button>
      </div>
    </div>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: 0,
    marginLeft: "auto",
    paddingRight: "0.25rem",
    position: "relative",
    zIndex: 10,
  },
  bodyFrame: {
    flex: "1",
    marginTop: "0.5rem",
    minHeight: 0,
    overflow: "hidden",
    position: "relative",
  },
  close: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  externalLink: {
    color: {
      default: "rgb(37 99 235)",
      ":hover": "rgb(29 78 216)",
      ":is(.dark *)": "rgb(96 165 250)",
      ":is(.dark *):hover": "rgb(147 197 253)",
    },
    textDecorationColor: {
      default: "rgb(96 165 250 / 0.4)",
      ":is(.dark *)": "rgb(59 130 246 / 0.5)",
    },
    textDecorationLine: "underline",
    textUnderlineOffset: "2px",
  },
  header: {
    alignItems: "center",
    display: "flex",
    height: "3rem",
    position: "relative",
    width: "100%",
  },
  headerFrame: {
    flexShrink: 0,
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
  },
  headerGutter: {
    paddingLeft: "80px",
  },
  headerMacosGutter: {
    paddingLeft: "156px",
  },
  muted: {
    color: colors.mutedForeground,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  scroll: {
    height: "100%",
    overflowY: "auto",
    paddingBottom: "1rem",
    paddingInline: "0.75rem",
  },
  title: {
    color: colors.foreground,
    fontSize: "1.25rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  titleCentered: {
    textAlign: "center",
  },
  titleLeft: {
    textAlign: "left",
  },
  titleSlot: {
    alignItems: "center",
    bottom: 0,
    display: "flex",
    pointerEvents: "none",
    position: "absolute",
    top: 0,
  },
  titleSlotCentered: {
    justifyContent: "center",
    left: "50%",
    transform: "translateX(-50%)",
    width: "min(640px, calc(100% - 160px))",
  },
  titleSlotExpanded: {
    justifyContent: "flex-start",
    left: 0,
    right: "70px",
  },
  titleSlotGutter: {
    justifyContent: "flex-start",
    right: "70px",
  },
  titleSlotMacos: {
    left: "104px",
  },
  titleSlotStandard: {
    left: "28px",
  },
});

export { styles as changelogStyles };
