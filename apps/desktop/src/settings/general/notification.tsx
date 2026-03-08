import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  commands as detectCommands,
  type InstalledApp,
  type Result,
} from "@hypr/plugin-detect";
import { commands as notificationCommands } from "@hypr/plugin-notification";
import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

export function NotificationSettingsView() {
  const [inputValue, setInputValue] = useState("");
  const [showDropdown, setShowDropdown] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const configs = useConfigValues([
    "notification_event",
    "notification_detect",
    "respect_dnd",
    "ignored_platforms",
    "mic_active_threshold",
  ] as const);

  useEffect(() => {
    void notificationCommands.clearNotifications();
    return () => {
      void notificationCommands.clearNotifications();
    };
  }, []);

  const { data: allInstalledApps } = useQuery({
    queryKey: ["settings", "all-installed-applications"],
    queryFn: detectCommands.listInstalledApplications,
    select: (result: Result<InstalledApp[], string>) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const { data: defaultIgnoredBundleIds } = useQuery({
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
    return allInstalledApps?.find((a) => a.id === bundleId)?.name ?? bundleId;
  };

  const nameToBundleId = (name: string) => {
    return allInstalledApps?.find((a) => a.name === name)?.id ?? name;
  };

  const isDefaultIgnored = (appName: string) => {
    const bundleId = nameToBundleId(appName);
    return defaultIgnoredBundleIds?.includes(bundleId) ?? false;
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
      ignored_platforms: configs.ignored_platforms.map(bundleIdToName),
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
      handleSetIgnoredPlatforms(
        JSON.stringify(value.ignored_platforms.map(nameToBundleId)),
      );
      handleSetMicActiveThreshold(value.mic_active_threshold);
    },
  });

  const anyNotificationEnabled =
    configs.notification_event || configs.notification_detect;
  const ignoredPlatforms = form.getFieldValue("ignored_platforms");

  const installedApps = allInstalledApps?.map((app) => app.name) ?? [];

  const filteredApps = installedApps.filter((app) => {
    const matchesSearch = app.toLowerCase().includes(inputValue.toLowerCase());
    const notAlreadyAdded = !ignoredPlatforms.includes(app);
    const notDefaultIgnored = !isDefaultIgnored(app);
    return matchesSearch && notAlreadyAdded && notDefaultIgnored;
  });

  const showCustomOption =
    inputValue.trim() &&
    !filteredApps.some((app) => app.toLowerCase() === inputValue.toLowerCase());

  const dropdownOptions = showCustomOption
    ? [inputValue.trim(), ...filteredApps]
    : filteredApps;

  const handleAddIgnoredApp = (appName: string) => {
    const trimmedName = appName.trim();
    if (
      !trimmedName ||
      ignoredPlatforms.includes(trimmedName) ||
      isDefaultIgnored(trimmedName)
    ) {
      return;
    }

    form.setFieldValue("ignored_platforms", [...ignoredPlatforms, trimmedName]);
    void form.handleSubmit();
    setInputValue("");
    setShowDropdown(false);
    setSelectedIndex(0);
  };

  const handleRemoveIgnoredApp = (app: string) => {
    const updated = ignoredPlatforms.filter((a: string) => a !== app);
    form.setFieldValue("ignored_platforms", updated);
    void form.handleSubmit();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && inputValue.trim()) {
      e.preventDefault();
      if (dropdownOptions.length > 0) {
        handleAddIgnoredApp(dropdownOptions[selectedIndex]);
      }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) =>
        prev < dropdownOptions.length - 1 ? prev + 1 : prev,
      );
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
    } else if (e.key === "Escape") {
      setShowDropdown(false);
      setSelectedIndex(0);
    } else if (
      e.key === "Backspace" &&
      !inputValue &&
      ignoredPlatforms.length > 0
    ) {
      const lastApp = ignoredPlatforms[ignoredPlatforms.length - 1];
      if (!isDefaultIgnored(lastApp)) {
        handleRemoveIgnoredApp(lastApp);
      }
    }
  };

  const handleInputChange = (value: string) => {
    setInputValue(value);
    setShowDropdown(true);
    setSelectedIndex(0);
  };

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setShowDropdown(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

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
                    These apps will not trigger detection.
                  </p>
                </div>
                <div className="relative" ref={containerRef}>
                  <div
                    className="flex min-h-[38px] w-full cursor-text flex-wrap items-center gap-2 rounded-md border p-2"
                    onClick={() => inputRef.current?.focus()}
                  >
                    {ignoredPlatforms.map((app: string) => {
                      const isDefault = isDefaultIgnored(app);
                      return (
                        <Badge
                          key={app}
                          variant="secondary"
                          className={cn([
                            "flex items-center gap-1 px-2 py-0.5 text-xs",
                            isDefault
                              ? ["bg-neutral-200 text-neutral-700"]
                              : ["bg-muted"],
                          ])}
                          title={isDefault ? "default" : undefined}
                        >
                          {app}
                          {isDefault && (
                            <span className="text-[10px] opacity-70">
                              (default)
                            </span>
                          )}
                          {!isDefault && (
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              className="ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
                              onClick={() => handleRemoveIgnoredApp(app)}
                            >
                              <X className="h-2.5 w-2.5" />
                            </Button>
                          )}
                        </Badge>
                      );
                    })}
                    <input
                      ref={inputRef}
                      type="text"
                      className="placeholder:text-muted-foreground min-w-[120px] flex-1 bg-transparent text-sm outline-hidden"
                      placeholder={
                        ignoredPlatforms.length === 0
                          ? "Type to add apps..."
                          : ""
                      }
                      value={inputValue}
                      onChange={(e) => handleInputChange(e.target.value)}
                      onKeyDown={handleKeyDown}
                      onFocus={() => setShowDropdown(true)}
                    />
                  </div>
                  {showDropdown && dropdownOptions.length > 0 && (
                    <div className="bg-popover absolute z-50 mt-1 w-full overflow-hidden rounded-md border shadow-md">
                      <div className="max-h-[200px] overflow-auto py-1">
                        {dropdownOptions.map((app, index) => {
                          const isCustom = showCustomOption && index === 0;
                          return (
                            <button
                              key={app}
                              type="button"
                              className={cn([
                                "w-full px-3 py-1.5 text-left text-sm transition-colors",
                                "hover:bg-accent hover:text-accent-foreground",
                                selectedIndex === index &&
                                  "bg-accent text-accent-foreground",
                              ])}
                              onClick={() => handleAddIgnoredApp(app)}
                              onMouseEnter={() => setSelectedIndex(index)}
                            >
                              {isCustom ? (
                                <span>
                                  Add "
                                  <span className="font-medium">{app}</span>"
                                </span>
                              ) : (
                                app
                              )}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
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
