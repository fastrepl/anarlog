import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowUpRight,
  ArrowsClockwise,
  Bell,
  BookOpen,
  CalendarDots,
  CircleNotch,
  Code,
  DownloadSimple,
  FileText,
  Gear,
  Lightning,
  type Icon,
  Lock,
  MagnifyingGlass,
  ShieldCheck,
  Sparkle,
  Sun,
  User,
  Users,
  UsersThree,
  VideoCamera,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useState } from "react";

import { colors, radii, spacing } from "@anlg/design-system/tokens.stylex";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { useBillingAccess } from "~/auth/billing-context";
import { privacyMessages } from "~/settings/general/app-settings";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";
import { type SettingsTab, type TabInput, useTabs } from "~/store/zustand/tabs";

type SettingsNavItem =
  | {
      id: SettingsTab;
      label: string;
      icon: Icon;
      requiresPro?: boolean;
    }
  | {
      id: "automations" | "calendar" | "contacts" | "templates";
      label: string;
      icon: Icon;
      destination: TabInput;
      requiresPro?: boolean;
    };

type SettingsNavGroup = { label: string; items: SettingsNavItem[] };

export function SettingsNav() {
  const { i18n, t } = useLingui();
  const { isPro, upgradeToPro, isUpgradingToPro } = useBillingAccess();
  const workspaces = useMyWorkspacesWithMirror();
  const hasExistingWorkspace = (workspaces.data?.length ?? 0) > 0;
  const [search, setSearch] = useState("");
  const currentTab = useTabs((state) => state.currentTab);
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const openNew = useTabs((state) => state.openNew);

  const requestedTab =
    currentTab?.type === "settings" ? (currentTab.state.tab ?? "app") : "app";
  const activeTab = requestedTab === "audio" ? "meetings" : requestedTab;

  const setActiveTab = useCallback(
    (tab: SettingsTab) => {
      if (currentTab?.type === "settings") {
        updateSettingsTabState(currentTab, { tab });
      }
    },
    [currentTab, updateSettingsTabState],
  );

  const groups: SettingsNavGroup[] = [
    {
      label: t`App`,
      items: [
        { id: "app", label: t`General`, icon: Gear },
        { id: "account", label: t`Account`, icon: User },
        {
          id: "team",
          label: t`Team`,
          icon: UsersThree,
          requiresPro: !workspaces.isLoading && !hasExistingWorkspace,
        },
        { id: "appearance", label: t`Appearance`, icon: Sun },
        { id: "notifications", label: t`Notifications`, icon: Bell },
      ],
    },
    {
      label: t`Workspace`,
      items: [
        { id: "meetings", label: t`Meetings`, icon: VideoCamera },
        {
          id: "calendar",
          label: t`Calendar`,
          icon: CalendarDots,
          destination: { type: "calendar" },
        },
        {
          id: "contacts",
          label: t`Contacts`,
          icon: Users,
          destination: { type: "contacts" },
        },
        {
          id: "templates",
          label: t`Templates`,
          icon: FileText,
          destination: { type: "templates" },
        },
        {
          id: "automations",
          label: t`Automations`,
          icon: Lightning,
          destination: { type: "automations" },
          requiresPro: true,
        },
      ],
    },
    {
      label: "AI",
      items: [
        { id: "transcription", label: t`Transcription`, icon: Sparkle },
        { id: "intelligence", label: t`Intelligence`, icon: Sparkle },
        {
          id: "dictionary",
          label: t`Dictionary`,
          icon: BookOpen,
          requiresPro: true,
        },
      ],
    },
    {
      label: t`Data`,
      items: [
        {
          id: "sync",
          label: t`Sync`,
          icon: ArrowsClockwise,
          requiresPro: true,
        },
        { id: "imports", label: t`Imports`, icon: DownloadSimple },
      ],
    },
    {
      label: t`Advanced`,
      items: [
        {
          id: "privacy",
          label: i18n._(privacyMessages.title),
          icon: ShieldCheck,
        },
        { id: "permissions", label: t`Permissions`, icon: Lock },
        { id: "developers", label: t`Developers`, icon: Code },
      ],
    },
  ];

  const query = search.trim().toLowerCase();
  const visibleGroups = query
    ? groups
        .map((group) =>
          group.label.toLowerCase().includes(query)
            ? group
            : {
                ...group,
                items: group.items.filter((item) =>
                  item.label.toLowerCase().includes(query),
                ),
              },
        )
        .filter((group) => group.items.length > 0)
    : groups;

  return (
    <div {...stylex.props(styles.root)}>
      <CustomSidebarHeader />
      <div {...stylex.props(styles.searchSection)}>
        <div {...stylex.props(styles.searchContainer)}>
          <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setSearch("");
              }
            }}
            placeholder={t`Search settings...`}
            {...stylex.props(styles.searchInput)}
          />
          {search ? (
            <button
              type="button"
              onClick={() => setSearch("")}
              {...stylex.props(styles.clearButton)}
              aria-label={t`Clear search`}
            >
              <X {...stylex.props(styles.clearIcon)} />
            </button>
          ) : null}
        </div>
      </div>
      <div {...stylex.props(styles.scrollArea)}>
        <div {...stylex.props(styles.groups)}>
          {visibleGroups.length === 0 ? (
            <div {...stylex.props(styles.empty)}>
              <MagnifyingGlass size={32} {...stylex.props(styles.emptyIcon)} />
              <p {...stylex.props(styles.emptyText)}>
                <Trans>No results found.</Trans>
              </p>
            </div>
          ) : null}
          {visibleGroups.map((group) => (
            <div key={group.label} {...stylex.props(styles.group)}>
              <span {...stylex.props(styles.groupLabel)}>{group.label}</span>
              {group.items.map((item) => {
                const requiresPro = Boolean(item.requiresPro && !isPro);

                return (
                  <div
                    key={item.id}
                    {...stylex.props(styles.navRow, stylex.defaultMarker())}
                  >
                    <button
                      type="button"
                      aria-disabled={requiresPro}
                      onClick={() => {
                        if (requiresPro) return;

                        if ("destination" in item) {
                          openNew(item.destination);
                          return;
                        }

                        setActiveTab(item.id);
                      }}
                      {...stylex.props(
                        styles.navButton,
                        activeTab === item.id
                          ? styles.navButtonActive
                          : styles.navButtonIdle,
                        requiresPro && styles.navButtonLocked,
                      )}
                    >
                      <item.icon
                        size={15}
                        {...stylex.props(styles.navIcon)}
                        data-testid={`settings-nav-icon-${item.id}`}
                      />
                      <span
                        {...stylex.props(
                          styles.navContent,
                          requiresPro && styles.navContentLocked,
                        )}
                      >
                        <span {...stylex.props(styles.navLabel)}>
                          {item.label}
                        </span>
                        {requiresPro ? (
                          <Lock
                            aria-hidden
                            {...stylex.props(styles.lockIcon)}
                          />
                        ) : "destination" in item ? (
                          <ArrowUpRight
                            aria-hidden
                            {...stylex.props(styles.destinationIcon)}
                            data-testid={`settings-nav-destination-icon-${item.id}`}
                          />
                        ) : null}
                      </span>
                    </button>
                    {requiresPro ? (
                      <button
                        type="button"
                        onClick={upgradeToPro}
                        disabled={isUpgradingToPro}
                        {...stylex.props(styles.upgradeButton)}
                        aria-label={t`Upgrade to Pro for ${item.label}`}
                      >
                        {isUpgradingToPro ? (
                          <CircleNotch
                            {...stylex.props(styles.upgradeSpinner)}
                            aria-hidden
                          />
                        ) : null}
                        <Trans>Upgrade to Pro</Trans>
                      </button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  clearButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    height: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  clearIcon: {
    height: "1rem",
    width: "1rem",
  },
  destinationIcon: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  empty: {
    color: colors.mutedForeground,
    paddingBlock: spacing.xxl,
    paddingInline: spacing.md,
    textAlign: "center",
  },
  emptyIcon: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    marginBottom: spacing.sm,
    marginInline: "auto",
  },
  emptyText: {
    fontSize: "0.875rem",
  },
  group: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  groupLabel: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 60%, transparent)`,
    fontSize: "11px",
    fontWeight: 500,
    letterSpacing: "0.05em",
    paddingBottom: spacing.xs,
    paddingInline: spacing.md,
    textTransform: "uppercase",
  },
  groups: {
    display: "flex",
    flexDirection: "column",
    gap: "1.25rem",
    paddingBottom: spacing.sm,
  },
  lockIcon: {
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  navButton: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    fontSize: "0.875rem",
    gap: spacing.sm,
    paddingBlock: spacing.sm,
    paddingInline: spacing.md,
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  navButtonActive: {
    backgroundColor: colors.sidebarAccent,
    color: colors.foreground,
    fontWeight: 500,
  },
  navButtonIdle: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.sidebarAccent} 50%, transparent)`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  navButtonLocked: {
    opacity: 0.6,
  },
  navContent: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: spacing.sm,
    minWidth: 0,
    transitionDuration: "150ms",
    transitionProperty: "opacity",
  },
  navContentLocked: {
    opacity: {
      default: 1,
      [stylex.when.ancestor(":focus-within")]: 0,
      [stylex.when.ancestor(":hover")]: 0,
    },
  },
  navIcon: {
    flexShrink: 0,
  },
  navLabel: {
    flex: "1",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  navRow: {
    position: "relative",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    width: "100%",
  },
  scrollArea: {
    "::-webkit-scrollbar": {
      display: "none",
    },
    flex: "1",
    overflowY: "auto",
    scrollbarWidth: "none",
  },
  searchContainer: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.accent} 50%, transparent)`,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexShrink: 0,
    gap: spacing.sm,
    height: "2rem",
    paddingInline: spacing.md,
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  searchInput: {
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    minWidth: 0,
    outline: {
      default: null,
      ":focus": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus": "2px",
    },
    "::placeholder": {
      color: colors.mutedForeground,
      fontSize: "0.875rem",
    },
  },
  searchSection: {
    paddingBottom: spacing.sm,
  },
  upgradeButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderColor: colors.primary,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow: {
      default: "0 4px 14px rgba(87, 83, 78, 0.18)",
      ":focus-visible": `0 0 0 2px ${colors.ring}, 0 4px 14px rgba(87, 83, 78, 0.18)`,
    },
    color: colors.primaryForeground,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: spacing.xs,
    opacity: {
      default: 0,
      ":disabled": 0.7,
      [stylex.when.ancestor(":focus-within")]: 1,
      [stylex.when.ancestor(":hover")]: 1,
    },
    outline: {
      default: "none",
      ":focus-visible": "2px solid transparent",
    },
    paddingBlock: spacing.xs,
    paddingInline: spacing.md,
    pointerEvents: {
      default: "none",
      [stylex.when.ancestor(":focus-within")]: "auto",
      [stylex.when.ancestor(":hover")]: "auto",
    },
    position: "absolute",
    right: spacing.xs,
    top: "50%",
    transform: {
      default: "translate(0.25rem, -50%)",
      [stylex.when.ancestor(":focus-within")]: "translate(0, -50%)",
      [stylex.when.ancestor(":hover")]: "translate(0, -50%)",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  upgradeSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: spacing.md,
    width: spacing.md,
  },
});
