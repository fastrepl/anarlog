import { events as appleCalendarEvents } from "@hypr/plugin-calendar";
import { events as deeplinkEvents } from "@hypr/plugin-deeplink2";
import { events as detectEvents } from "@hypr/plugin-detect";
import { events as listenerEvents } from "@hypr/plugin-listener";
import { events as listener2Events } from "@hypr/plugin-listener2";
import { events as localSttEvents } from "@hypr/plugin-local-stt";
import { events as networkEvents } from "@hypr/plugin-network";
import { events as notificationEvents } from "@hypr/plugin-notification";
import { events as notifyEvents } from "@hypr/plugin-notify";
import { events as updaterEvents } from "@hypr/plugin-updater2";
import { events as windowsEvents } from "@hypr/plugin-windows";

export const pluginEvents = {
  tauri: {
    appleCalendar: appleCalendarEvents,
    deeplink2: deeplinkEvents,
    detect: detectEvents,
    listener: listenerEvents,
    listener2: listener2Events,
    localStt: localSttEvents,
    network: networkEvents,
    notification: notificationEvents,
    notify: notifyEvents,
    updater2: updaterEvents,
    windows: windowsEvents,
  },
};
