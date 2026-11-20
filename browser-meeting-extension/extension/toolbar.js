(function installAnarlogToolbar(global) {
  "use strict";
  const api = global.chrome;
  const meeting = global.AnarlogMeetingBackground;
  const controller = global.AnarlogMeetingController;
  if (!api?.action || !meeting || !controller) return;

  const installMenus = async () => {
    await api.contextMenus.removeAll();
    api.contextMenus.create({
      id: meeting.MENU.stop,
      title: "Stop recording",
      contexts: ["action"],
      enabled: controller.active(),
    });
  };
  api.runtime.onInstalled.addListener(() => void installMenus());
  api.runtime.onStartup.addListener(() => void installMenus());

  // The click is what grants activeTab: only a Meet call the person clicked is ever read.
  api.action.onClicked.addListener((tab) => {
    if (meeting.meetingUrl(tab?.url)) {
      void api.action.setBadgeText({ tabId: tab.id, text: null });
      void api.action.setTitle({ tabId: tab.id, title: "" });
      controller.toolbar(tab);
      return;
    }
    if (!Number.isInteger(tab?.id)) return;
    void api.action.setBadgeText({ tabId: tab.id, text: "!" });
    void api.action.setTitle({
      tabId: tab.id,
      title: "Open a Google Meet call, then click Anarlog to record it.",
    });
  });
  api.tabs.onUpdated.addListener((id, change) => {
    if (change.status === "loading" || change.url) {
      void api.action.setBadgeText({ tabId: id, text: null });
      void api.action.setTitle({ tabId: id, title: "" });
    }
  });
})(globalThis);
