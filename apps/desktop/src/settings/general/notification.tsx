import { Trans, useLingui } from "@lingui/react/macro";
import { X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import {
  commands as detectCommands,
  type InstalledApp,
  type Result,
} from "@anlg/plugin-detect";
import { commands as notificationCommands } from "@anlg/plugin-notification";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@anlg/ui/components/ui/command";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import {
  getIgnoredBundleIds,
  getIgnorableApps,
  toggleIgnoredApp,
} from "./notification-app-options";

import { useSetSettingValues } from "~/settings/queries";
import { SettingSwitchRow } from "~/settings/setting-row";
import { useConfigValues } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function NotificationSettingsView() {
  const { t } = useLingui();
  const currentPlatform = platform();
  const supportsMicDetection = currentPlatform !== "windows";
  const supportsDoNotDisturb = currentPlatform === "macos";
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const configs = useConfigValues([
    "notification_event",
    "notification_detect",
    "notification_bounce",
    "show_app_in_dock",
    "respect_dnd",
    "ignored_platforms",
    "included_platforms",
    "mic_active_threshold",
  ] as const);

  useMountEffect(() => {
    void notificationCommands.clearNotifications();
    return () => {
      void notificationCommands.clearNotifications();
    };
  });

  const { data: installedApps = [] } = useQuery({
    queryKey: ["settings", "all-installed-applications"],
    queryFn: detectCommands.listInstalledApplications,
    enabled: supportsMicDetection,
    select: (result: Result<InstalledApp[], string>) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const { data: defaultIgnoredBundleIds = [] } = useQuery({
    queryKey: ["settings", "default-ignored-bundle-ids"],
    queryFn: detectCommands.listDefaultIgnoredBundleIds,
    enabled: supportsMicDetection,
    select: (result: Result<string[], string>) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const bundleIdToName = (bundleId: string) => {
    return installedApps.find((a) => a.id === bundleId)?.name ?? bundleId;
  };

  const isDefaultIgnored = (bundleId: string) => {
    return defaultIgnoredBundleIds.includes(bundleId);
  };

  const setSettingValues = useSetSettingValues();

  const form = useForm({
    defaultValues: {
      notification_event: configs.notification_event,
      notification_detect: configs.notification_detect,
      notification_bounce: configs.notification_bounce,
      respect_dnd: configs.respect_dnd,
      ignored_platforms: configs.ignored_platforms,
      included_platforms: configs.included_platforms,
      mic_active_threshold: configs.mic_active_threshold,
    },
    listeners: {
      onChange: async ({ formApi }) => {
        void formApi.handleSubmit();
      },
    },
    onSubmit: async ({ value }) => {
      setSettingValues({
        notification_event: value.notification_event,
        notification_detect: value.notification_detect,
        notification_bounce: value.notification_bounce,
        respect_dnd: value.respect_dnd,
        ignored_platforms: JSON.stringify(value.ignored_platforms),
        included_platforms: JSON.stringify(value.included_platforms),
        mic_active_threshold: value.mic_active_threshold,
      });
    },
  });

  const handleToggleIgnoredApp = (
    bundleId: string,
    ignoredPlatforms: string[],
    includedPlatforms: string[],
  ) => {
    if (!bundleId) {
      return;
    }

    const {
      ignoredPlatforms: newIgnoredPlatforms,
      includedPlatforms: newIncludedPlatforms,
    } = toggleIgnoredApp({
      bundleId,
      ignoredPlatforms,
      includedPlatforms,
      defaultIgnoredBundleIds,
    });

    form.setFieldValue("ignored_platforms", newIgnoredPlatforms);
    form.setFieldValue("included_platforms", newIncludedPlatforms);
    void form.handleSubmit();
    setSearchOpen(false);
    setSearchQuery("");
  };

  return (
    <div {...stylex.props(styles.stack)}>
      <form.Field name="notification_event">
        {(field) => (
          <SettingSwitchRow
            title={<Trans>Event notifications</Trans>}
            description={
              <Trans>Prepare for events with a 5-minute reminder.</Trans>
            }
            checked={field.state.value}
            onChange={field.handleChange}
          />
        )}
      </form.Field>

      {(currentPlatform !== "macos" || configs.show_app_in_dock) && (
        <form.Field name="notification_bounce">
          {(field) => (
            <SettingSwitchRow
              title={<Trans>Bounce app icon</Trans>}
              description={
                <Trans>
                  Get your attention when Anarlog finishes work in the
                  background.
                </Trans>
              }
              checked={field.state.value}
              onChange={field.handleChange}
            />
          )}
        </form.Field>
      )}

      {supportsMicDetection && (
        <form.Field name="notification_detect">
          {(field) => (
            <div {...stylex.props(styles.micSettings)}>
              <SettingSwitchRow
                title={<Trans>Microphone detection</Trans>}
                description={
                  <Trans>Detect meetings from microphone activity.</Trans>
                }
                checked={field.state.value}
                onChange={field.handleChange}
              />

              {field.state.value && (
                <div {...stylex.props(styles.nestedSettings)}>
                  <form.Field name="mic_active_threshold">
                    {(thresholdField) => (
                      <div {...stylex.props(styles.thresholdRow)}>
                        <div {...stylex.props(styles.flexible)}>
                          <h4 {...stylex.props(styles.rowTitle)}>
                            <Trans>Detection delay</Trans>
                          </h4>
                          <p {...stylex.props(styles.rowDescription)}>
                            <Trans>
                              Wait before treating microphone activity as a
                              meeting.
                            </Trans>
                          </p>
                        </div>
                        <Select
                          value={String(thresholdField.state.value)}
                          onValueChange={(v) =>
                            thresholdField.handleChange(Number(v))
                          }
                        >
                          <SelectTrigger sx={styles.thresholdSelect}>
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent align="end">
                            <SelectItem value="5">5 sec</SelectItem>
                            <SelectItem value="10">10 sec</SelectItem>
                            <SelectItem value="15">15 sec</SelectItem>
                            <SelectItem value="30">30 sec</SelectItem>
                            <SelectItem value="60">1 min</SelectItem>
                            <SelectItem value="120">2 min</SelectItem>
                          </SelectContent>
                        </Select>
                      </div>
                    )}
                  </form.Field>

                  <div {...stylex.props(styles.excludeHeading)}>
                    <h4 {...stylex.props(styles.rowTitle)}>
                      <Trans>Exclude apps from detection</Trans>
                    </h4>
                    <p {...stylex.props(styles.rowDescription)}>
                      <Trans>
                        Prevent selected apps from triggering meeting detection.
                      </Trans>
                    </p>
                  </div>
                  <form.Subscribe selector={(state) => state.values}>
                    {(values) => {
                      const ignoredPlatforms = values.ignored_platforms;
                      const includedPlatforms = values.included_platforms;
                      const ignorableApps = getIgnorableApps({
                        installedApps,
                        ignoredPlatforms,
                        includedPlatforms,
                        inputValue: searchQuery,
                        defaultIgnoredBundleIds,
                      });
                      const ignoredBundleIds = getIgnoredBundleIds({
                        installedApps,
                        ignoredPlatforms,
                        includedPlatforms,
                        defaultIgnoredBundleIds,
                      });

                      return (
                        <div {...stylex.props(styles.appSelector)}>
                          <Popover
                            open={searchOpen}
                            onOpenChange={setSearchOpen}
                          >
                            <PopoverTrigger asChild>
                              <div
                                role="button"
                                tabIndex={0}
                                aria-expanded={searchOpen}
                                {...stylex.props(styles.appSelectorTrigger)}
                                onKeyDown={(event) => {
                                  if (
                                    event.key === "Enter" ||
                                    event.key === " "
                                  ) {
                                    event.preventDefault();
                                    setSearchOpen(true);
                                  }
                                }}
                              >
                                {ignoredBundleIds.map((bundleId: string) => {
                                  const isDefault = isDefaultIgnored(bundleId);
                                  return (
                                    <Badge
                                      key={bundleId}
                                      variant="secondary"
                                      sx={[
                                        styles.appBadge,
                                        isDefault
                                          ? styles.defaultBadge
                                          : styles.customBadge,
                                      ]}
                                      title={isDefault ? "default" : undefined}
                                    >
                                      {bundleIdToName(bundleId)}
                                      {isDefault && (
                                        <span
                                          {...stylex.props(styles.defaultLabel)}
                                        >
                                          <Trans>(default)</Trans>
                                        </span>
                                      )}
                                      <Button
                                        type="button"
                                        variant="ghost"
                                        size="sm"
                                        sx={styles.removeButton}
                                        onClick={(event) => {
                                          event.stopPropagation();
                                          handleToggleIgnoredApp(
                                            bundleId,
                                            ignoredPlatforms,
                                            includedPlatforms,
                                          );
                                        }}
                                      >
                                        <X
                                          {...stylex.props(styles.removeIcon)}
                                        />
                                      </Button>
                                    </Badge>
                                  );
                                })}
                                <span
                                  {...stylex.props(styles.searchPlaceholder)}
                                >
                                  <Trans>Search installed apps...</Trans>
                                </span>
                              </div>
                            </PopoverTrigger>
                            <PopoverContent
                              variant="app"
                              align="start"
                              style={{
                                width: "var(--radix-popover-trigger-width)",
                              }}
                            >
                              <AppFloatingPanel sx={styles.panel}>
                                <Command sx={styles.command}>
                                  <CommandInput
                                    placeholder={t`Search installed apps...`}
                                    value={searchQuery}
                                    onValueChange={setSearchQuery}
                                  />
                                  <CommandEmpty>
                                    <div {...stylex.props(styles.empty)}>
                                      <Trans>No apps found.</Trans>
                                    </div>
                                  </CommandEmpty>
                                  <CommandList>
                                    <CommandGroup sx={styles.commandGroup}>
                                      {ignorableApps.map((app) => (
                                        <CommandItem
                                          key={app.id}
                                          value={`${app.name} ${app.id}`}
                                          onSelect={() =>
                                            handleToggleIgnoredApp(
                                              app.id,
                                              ignoredPlatforms,
                                              includedPlatforms,
                                            )
                                          }
                                          sx={styles.commandItem}
                                        >
                                          <span
                                            {...stylex.props(styles.appName)}
                                          >
                                            {app.name}
                                          </span>
                                        </CommandItem>
                                      ))}
                                    </CommandGroup>
                                  </CommandList>
                                </Command>
                              </AppFloatingPanel>
                            </PopoverContent>
                          </Popover>
                        </div>
                      );
                    }}
                  </form.Subscribe>
                </div>
              )}
            </div>
          )}
        </form.Field>
      )}

      {supportsDoNotDisturb && (
        <div {...stylex.props(styles.stack)}>
          <div {...stylex.props(styles.dividerRow)}>
            <div {...stylex.props(styles.divider)} />
            <span {...stylex.props(styles.dividerLabel)}>
              <Trans>For enabled notifications</Trans>
            </span>
            <div {...stylex.props(styles.divider)} />
          </div>

          <form.Subscribe
            selector={(state) =>
              state.values.notification_event ||
              state.values.notification_detect
            }
          >
            {(anyNotificationEnabled) => (
              <form.Field name="respect_dnd">
                {(field) => (
                  <SettingSwitchRow
                    title={<Trans>Respect Do-Not-Disturb mode</Trans>}
                    description={
                      <Trans>Pause alerts while Do Not Disturb is on.</Trans>
                    }
                    checked={field.state.value}
                    onChange={field.handleChange}
                    disabled={!anyNotificationEnabled}
                  />
                )}
              </form.Field>
            )}
          </form.Subscribe>
        </div>
      )}
    </div>
  );
}

const styles = stylex.create({
  appBadge: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
  },
  appName: {
    flex: "1",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  appSelector: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  appSelectorTrigger: {
    alignItems: "center",
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 1px ${colors.ring}`,
    },
    cursor: "text",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
    minHeight: "38px",
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    padding: "0.5rem",
    width: "100%",
  },
  command: {
    backgroundColor: "transparent",
    borderWidth: 0,
    borderRadius: "inherit",
  },
  commandGroup: {
    maxHeight: "250px",
    overflowY: "auto",
  },
  commandItem: {
    backgroundColor: {
      default: null,
      ":hover": colors.accent,
      ":focus": colors.accent,
      ":is([aria-selected='true'])": "transparent",
    },
    cursor: "pointer",
  },
  customBadge: {
    backgroundColor: colors.muted,
  },
  defaultBadge: {
    backgroundColor: colors.accent,
    color: colors.mutedForeground,
  },
  defaultLabel: {
    fontSize: "10px",
    opacity: 0.7,
  },
  divider: {
    borderTopColor: colors.muted,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    flex: "1",
    minWidth: 0,
  },
  dividerLabel: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  dividerRow: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    paddingBottom: "0.5rem",
    paddingTop: "1rem",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
  },
  excludeHeading: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
    marginBottom: "0.75rem",
  },
  flexible: {
    flex: "1",
  },
  micSettings: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  nestedSettings: {
    borderLeftColor: colors.muted,
    borderLeftStyle: "solid",
    borderLeftWidth: "2px",
    marginLeft: "0.75rem",
    paddingLeft: "1rem",
    paddingTop: "0.5rem",
  },
  panel: {
    overflow: "hidden",
  },
  removeButton: {
    backgroundColor: {
      default: null,
      ":hover": "transparent",
    },
    height: "0.75rem",
    marginLeft: "0.125rem",
    padding: 0,
    width: "0.75rem",
  },
  removeIcon: {
    height: "0.625rem",
    width: "0.625rem",
  },
  rowDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  rowTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  searchPlaceholder: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  stack: {
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
  },
  thresholdRow: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
    marginBottom: "1rem",
  },
  thresholdSelect: {
    width: "100px",
  },
});
