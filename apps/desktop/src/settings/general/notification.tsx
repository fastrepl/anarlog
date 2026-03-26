import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useEffect, useState } from "react";

import {
  commands as detectCommands,
  type InstalledApp,
  type Result,
} from "@hypr/plugin-detect";
import { commands as notificationCommands } from "@hypr/plugin-notification";
import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@hypr/ui/components/ui/command";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import {
  getIgnoredBundleIds,
  getIgnorableApps,
  toggleIgnoredApp,
} from "./notification-app-options";

import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

export function NotificationSettingsView() {
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const configs = useConfigValues([
    "notification_event",
    "notification_detect",
    "respect_dnd",
    "ignored_platforms",
    "included_platforms",
    "mic_active_threshold",
  ] as const);

  useEffect(() => {
    void notificationCommands.clearNotifications();
    return () => {
      void notificationCommands.clearNotifications();
    };
  }, []);

  const { data: installedApps = [] } = useQuery({
    queryKey: ["settings", "all-installed-applications"],
    queryFn: detectCommands.listInstalledApplications,
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

  const handleSetNotificationEvent = settings.UI.useSetValueCallback(
    "notification_event",
    (value: boolean) => value,
    [],
    settings.STORE_ID,
  );

  const handleSetNotificationDetect = settings.UI.useSetValueCallback(
    "notification_detect",
    (value: boolean) => value,
    [],
    settings.STORE_ID,
  );

  const handleSetRespectDnd = settings.UI.useSetValueCallback(
    "respect_dnd",
    (value: boolean) => value,
    [],
    settings.STORE_ID,
  );

  const handleSetIgnoredPlatforms = settings.UI.useSetValueCallback(
    "ignored_platforms",
    (value: string) => value,
    [],
    settings.STORE_ID,
  );

  const handleSetIncludedPlatforms = settings.UI.useSetValueCallback(
    "included_platforms",
    (value: string) => value,
    [],
    settings.STORE_ID,
  );

  const handleSetMicActiveThreshold = settings.UI.useSetValueCallback(
    "mic_active_threshold",
    (value: number) => value,
    [],
    settings.STORE_ID,
  );

  const form = useForm({
    defaultValues: {
      notification_event: configs.notification_event,
      notification_detect: configs.notification_detect,
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
      handleSetNotificationEvent(value.notification_event);
      handleSetNotificationDetect(value.notification_detect);
      handleSetRespectDnd(value.respect_dnd);
      handleSetIgnoredPlatforms(JSON.stringify(value.ignored_platforms));
      handleSetIncludedPlatforms(JSON.stringify(value.included_platforms));
      handleSetMicActiveThreshold(value.mic_active_threshold);
    },
  });

  const anyNotificationEnabled =
    configs.notification_event || configs.notification_detect;
  const ignoredPlatforms = form.getFieldValue("ignored_platforms");
  const includedPlatforms = form.getFieldValue("included_platforms");

  const ignorableApps = getIgnorableApps({
    installedApps,
    ignoredPlatforms,
    includedPlatforms,
    inputValue: searchQuery,
    defaultIgnoredBundleIds,
  });
  const ignoredBundleIds = getIgnoredBundleIds({
    installedApps: installedApps,
    ignoredPlatforms,
    includedPlatforms,
    defaultIgnoredBundleIds,
  });

  const handleToggleIgnoredApp = (bundleId: string) => {
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
    <div className="flex flex-col gap-6">
      <form.Field name="notification_event">
        {(field) => (
          <div className="flex items-start justify-between gap-4">
            <div className="flex-1">
              <h3 className="mb-1 text-sm font-medium">Event notifications</h3>
              <p className="text-xs text-neutral-600">
                Get notified 5 minutes before calendar events start
              </p>
            </div>
            <Switch
              checked={field.state.value}
              onCheckedChange={field.handleChange}
            />
          </div>
        )}
      </form.Field>

      <form.Field name="notification_detect">
        {(field) => (
          <div className="flex flex-col gap-4">
            <div className="flex items-start justify-between gap-4">
              <div className="flex-1">
                <h3 className="mb-1 text-sm font-medium">
                  Microphone detection
                </h3>
                <p className="text-xs text-neutral-600">
                  Automatically detect when a meeting starts based on microphone
                  activity.
                </p>
              </div>
              <Switch
                checked={field.state.value}
                onCheckedChange={field.handleChange}
              />
            </div>

            {field.state.value && (
              <div className={cn(["border-muted ml-6 border-l-2 pt-2 pl-6"])}>
                <form.Field name="mic_active_threshold">
                  {(thresholdField) => (
                    <div className="mb-4 flex items-center justify-between gap-4">
                      <div className="flex-1">
                        <h4 className="text-sm font-medium">Detection delay</h4>
                        <p className="text-xs text-neutral-600">
                          How long the mic must be active before triggering
                        </p>
                      </div>
                      <Select
                        value={String(thresholdField.state.value)}
                        onValueChange={(v) =>
                          thresholdField.handleChange(Number(v))
                        }
                      >
                        <SelectTrigger className="w-[100px]">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="5">5 sec</SelectItem>
                          <SelectItem value="10">10 sec</SelectItem>
                          <SelectItem value="15">15 sec</SelectItem>
                          <SelectItem value="30">30 sec</SelectItem>
                          <SelectItem value="60">60 sec</SelectItem>
                          <SelectItem value="120">120 sec</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                  )}
                </form.Field>

                <div className="mb-3 flex flex-col gap-1">
                  <h4 className="text-sm font-medium">
                    Exclude apps from detection
                  </h4>
                  <p className="text-xs text-neutral-600">
                    Search installed apps to exclude them. Click an excluded app
                    to include it again.
                  </p>
                </div>
                <div className="flex flex-col gap-3">
                  <Popover open={searchOpen} onOpenChange={setSearchOpen}>
                    <PopoverTrigger asChild>
                      <div
                        role="button"
                        tabIndex={0}
                        aria-expanded={searchOpen}
                        className={cn([
                          "flex min-h-[38px] w-full cursor-text flex-wrap items-center gap-2 rounded-md border p-2",
                          "focus-visible:ring-ring focus-visible:ring-1 focus-visible:outline-hidden",
                        ])}
                        onKeyDown={(event) => {
                          if (event.key === "Enter" || event.key === " ") {
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
                              className={cn([
                                "flex items-center gap-1 px-2 py-0.5 text-xs",
                                isDefault
                                  ? ["bg-neutral-200 text-neutral-700"]
                                  : ["bg-muted"],
                              ])}
                              title={isDefault ? "default" : undefined}
                            >
                              {bundleIdToName(bundleId)}
                              {isDefault && (
                                <span className="text-[10px] opacity-70">
                                  (default)
                                </span>
                              )}
                              <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
                                onClick={(event) => {
                                  event.stopPropagation();
                                  handleToggleIgnoredApp(bundleId);
                                }}
                              >
                                <X className="h-2.5 w-2.5" />
                              </Button>
                            </Badge>
                          );
                        })}
                        <span className="text-muted-foreground text-sm">
                          Search installed apps...
                        </span>
                      </div>
                    </PopoverTrigger>
                    <PopoverContent
                      className="p-0"
                      align="start"
                      style={{ width: "var(--radix-popover-trigger-width)" }}
                    >
                      <Command>
                        <CommandInput
                          placeholder="Search installed apps..."
                          value={searchQuery}
                          onValueChange={setSearchQuery}
                        />
                        <CommandEmpty>
                          <div className="text-muted-foreground px-2 py-1.5 text-sm">
                            No apps found.
                          </div>
                        </CommandEmpty>
                        <CommandList>
                          <CommandGroup className="max-h-[250px] overflow-y-auto">
                            {ignorableApps.map((app) => (
                              <CommandItem
                                key={app.id}
                                value={`${app.name} ${app.id}`}
                                onSelect={() => handleToggleIgnoredApp(app.id)}
                                className={cn([
                                  "cursor-pointer",
                                  "hover:bg-neutral-200! focus:bg-neutral-200! aria-selected:bg-transparent",
                                ])}
                              >
                                <span className="flex-1 truncate">
                                  {app.name}
                                </span>
                              </CommandItem>
                            ))}
                          </CommandGroup>
                        </CommandList>
                      </Command>
                    </PopoverContent>
                  </Popover>
                </div>
              </div>
            )}
          </div>
        )}
      </form.Field>

      <div className="flex flex-col gap-6">
        <div className="relative flex items-center pt-4 pb-2">
          <div className="border-muted w-full border-t" />
          <span className="bg-background text-muted-foreground absolute left-1/2 -translate-x-1/2 px-4 text-xs font-medium">
            For enabled notifications
          </span>
        </div>

        <form.Field name="respect_dnd">
          {(field) => (
            <div className="flex items-start justify-between gap-4">
              <div className="flex-1">
                <h3 className="mb-1 text-sm font-medium">
                  Respect Do-Not-Disturb mode
                </h3>
                <p className="text-xs text-neutral-600">
                  Don't show notifications when Do-Not-Disturb is enabled on
                  your system
                </p>
              </div>
              <Switch
                checked={field.state.value}
                onCheckedChange={field.handleChange}
                disabled={!anyNotificationEnabled}
              />
            </div>
          )}
        </form.Field>
      </div>
    </div>
  );
}
