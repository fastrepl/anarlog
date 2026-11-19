(function installAnarlogMeetingBackground(global) {
  "use strict";

  const MENU = { stop: "anarlog-meeting-stop" };
  /** A host that never says it reached Anarlog is an old one, or Anarlog without the bridge. */
  const CONNECT_TIMEOUT_MS = 10_000;
  const SETUP = "Open Anarlog and turn on the browser extension in Settings.";

  /** The call's own address, `https://meet.google.com/abc-defg-hij`, or null for any other page. */
  function meetingUrl(value) {
    try {
      const url = new URL(value);
      const call = /^\/(?:meet\/)?([a-z]{3}-[a-z]{4}-[a-z]{3})$/.exec(url.pathname);
      return url.origin === "https://meet.google.com" && call ? `${url.origin}/${call[1]}` : null;
    } catch {
      return null;
    }
  }

  // Only a toolbar click starts, pauses, or resumes, and activeTab comes only from that click, so
  // the extension never reaches a Meet tab the person didn't click. Anarlog records in the app,
  // through its own record button's path. The native port stays open for the length of the hold,
  // which also keeps this service worker alive to watch the tab. When Anarlog's own window is
  // already recording this call, the click links the tab to that recording instead. A tab that
  // closes, navigates, can't be read, or shows the call ended lets go (`meeting_detach`): a
  // recording it started stops, and one it joined keeps going in the app. Only the person's own
  // Stop stops a recording the tab joined.
  function createController(api, host) {
    let port = null;
    let tabId = null;
    let url = null;
    let state = "idle";
    let pausable = false;
    let connectTimer = null;

    function show(next, title) {
      state = next;
      if (api.contextMenus?.update)
        void Promise.resolve(api.contextMenus.update(MENU.stop, { enabled: port !== null })).catch(
          () => {},
        );
      const text =
        { starting: "…", pausing: "…", resuming: "…", stopping: "…", leaving: "…", error: "!" }[
          next
        ] || "";
      void api.action.setBadgeText({ text });
      if (typeof OffscreenCanvas !== "undefined") {
        const canvas = new OffscreenCanvas(32, 32);
        const ctx = canvas.getContext("2d");
        if (next === "recording") {
          ctx.fillStyle = "#dc2626";
          ctx.beginPath();
          ctx.arc(16, 16, 11, 0, 2 * Math.PI);
          ctx.fill();
        } else if (next === "paused") {
          ctx.fillStyle = "#d97706";
          ctx.fillRect(7, 5, 6, 22);
          ctx.fillRect(19, 5, 6, 22);
        }
        void api.action.setIcon(
          next === "recording" || next === "paused"
            ? { imageData: ctx.getImageData(0, 0, 32, 32) }
            : { path: { 32: "icons/icon-32.png", 128: "icons/icon-128.png" } },
        );
      }
      const titles = {
        starting: "Starting a recording in Anarlog…",
        recording: pausable
          ? "Anarlog is recording this call. Click on this Meet tab to pause."
          : "Anarlog is recording this call. Stop it from this button's menu or in Anarlog.",
        paused: "Recording paused. Click on this Meet tab to resume.",
        stopping: "Stopping the recording…",
        leaving: "Letting go of this tab…",
      };
      void api.action.setTitle({ title: title || titles[next] || "Anarlog" });
    }

    // A notice still waiting for the person to join, or for Meet's chat, must not post once the
    // recording has stopped; the camera switch stops with it.
    function cancelNotice(id) {
      if (!api.scripting || !Number.isInteger(id)) return;
      void api.scripting
        .executeScript({
          target: { tabId: id },
          func: () => {
            globalThis.anarlogMeetingChat?.cancel();
            globalThis.anarlogMeetingCamera?.cancel();
          },
        })
        .catch(() => {});
    }

    // The link is over: the tab's page scripts stop reading too. A later click starts them again.
    function quietTab(id) {
      if (!api.scripting || !Number.isInteger(id)) return;
      void api.scripting
        .executeScript({
          target: { tabId: id },
          func: () => {
            globalThis.anarlogMeetingChat?.cancel();
            globalThis.anarlogMeetingCamera?.cancel();
            globalThis.anarlogMeetingSpeakers?.stop?.();
            globalThis.anarlogMeetingMonitor?.stop?.();
          },
        })
        .catch(() => {});
    }

    function release() {
      quietTab(tabId);
      clearTimeout(connectTimer);
      connectTimer = null;
      const connection = port;
      port = null;
      tabId = null;
      url = null;
      pausable = false;
      return connection;
    }

    function stop() {
      if (!port || state === "stopping") return;
      cancelNotice(tabId);
      show("stopping");
      port.postMessage({ type: "meeting_stop" });
    }

    // The tab is gone or unreadable; Anarlog decides whether that ends the recording.
    function leave() {
      if (!port || state === "stopping" || state === "leaving") return;
      cancelNotice(tabId);
      show("leaving");
      port.postMessage({ type: "meeting_detach" });
    }

    // "Message the meeting chat" in Anarlog: post the notice in this call's chat once. The page
    // script waits for the person to be in the call, and Anarlog hears how it went, so another tab
    // isn't handed a notice that may have gone out.
    function postNotice(id, notice, connection) {
      if (!api.scripting) return;
      void api.scripting
        .executeScript({ target: { tabId: id }, files: ["meeting-chat.js"] })
        .then(() =>
          api.scripting.executeScript({
            target: { tabId: id },
            func: (text) => globalThis.anarlogMeetingChat.post(text),
            args: [notice],
          }),
        )
        .then((results) => {
          const outcome = results && results[0] && results[0].result;
          if (port === connection && typeof outcome === "string")
            connection.postMessage({ type: "meeting_chat", outcome });
        })
        .catch(() => {});
    }

    // Automatic camera switching in Anarlog: pick Anarlog's camera in Meet's own settings, then
    // tell Anarlog how it went, so it can ask the person when it didn't.
    function switchCamera(id, camera, connection) {
      if (!api.scripting) return Promise.resolve();
      return api.scripting
        .executeScript({ target: { tabId: id }, files: ["meeting-camera.js"] })
        .then(() =>
          api.scripting.executeScript({
            target: { tabId: id },
            func: (name) => globalThis.anarlogMeetingCamera.select(name),
            args: [camera],
          }),
        )
        .then((results) => {
          const outcome = results && results[0] && results[0].result;
          if (port === connection && typeof outcome === "string")
            connection.postMessage({ type: "meeting_camera", outcome });
        })
        .catch(() => {});
    }

    function disconnect(connection, next, title) {
      release();
      show(next, title);
      connection.disconnect();
    }

    function start(tab) {
      if (port) return;
      const target = meetingUrl(tab && tab.url);
      if (!target || !Number.isInteger(tab.id)) {
        show("error", "Open a Google Meet call, then click Anarlog to record it.");
        return;
      }
      let connection;
      try {
        connection = api.runtime.connectNative(host);
      } catch {
        show("error", SETUP);
        return;
      }
      port = connection;
      tabId = tab.id;
      url = target;
      show("starting");
      connectTimer = setTimeout(() => {
        if (port === connection)
          disconnect(connection, "error", "Update Anarlog and the extension together.");
      }, CONNECT_TIMEOUT_MS);
      connection.onMessage.addListener((message) => {
        if (port !== connection || !message || typeof message !== "object") return;
        clearTimeout(connectTimer);
        if (message.event === "connected") return;
        // Anarlog said no to a pause, resume or stop: the recording stays as it was.
        if (message.event === "refused") {
          const settled = { pausing: "recording", resuming: "paused", stopping: "recording" };
          show(
            settled[state] || state,
            typeof message.message === "string" ? message.message : undefined,
          );
          return;
        }
        const wanted = state === "stopping" || state === "leaving" ? null : message.event;
        if (wanted === "started") pausable = message.pausable === true;
        if (wanted === "started" || wanted === "resumed") show("recording");
        if (wanted === "paused") show("paused");
        const notice =
          wanted === "started" && typeof message.notice === "string" && message.notice
            ? message.notice
            : null;
        // The camera switch works in Meet's own Settings dialog; the notice waits until it is
        // done, so the two never drive the same tab at once.
        if (wanted === "started" && typeof message.camera === "string" && message.camera)
          void switchCamera(tab.id, message.camera, connection).then(() => {
            if (notice && port === connection) postNotice(tab.id, notice, connection);
          });
        else if (notice) postNotice(tab.id, notice, connection);
        if (message.event === "ended" || message.event === "detached")
          disconnect(
            connection,
            "done",
            message.event === "ended"
              ? "Recording saved in Anarlog."
              : "Anarlog is still recording in the app.",
          );
        else if (message.event === "error")
          disconnect(
            connection,
            "error",
            typeof message.message === "string" && message.message
              ? message.message
              : "Anarlog couldn't record this meeting.",
          );
      });
      connection.onDisconnect.addListener(() => {
        // Reading lastError consumes Chrome's native-host diagnostic without exposing native paths
        // or command output to a web page.
        const error = api.runtime.lastError;
        if (port !== connection) return;
        release();
        show("error", error ? SETUP : "Anarlog stopped answering. Check the recording in Anarlog.");
      });
      connection.postMessage({ type: "meeting_start", url: target });
      if (api.scripting)
        void api.scripting
          .executeScript({
            target: { tabId: tab.id },
            files: ["meeting-content.js", "meeting-speakers.js"],
          })
          .catch(() => {
            if (port === connection) leave();
          });
    }

    function updated(id, change) {
      if (
        id === tabId &&
        (change.status === "loading" ||
          (typeof change.url === "string" && meetingUrl(change.url) !== url))
      )
        leave();
    }

    return {
      click: (info) => {
        if (info.menuItemId === MENU.stop) stop();
      },
      updated,
      toolbar: (tab) => {
        if (!port) {
          start(tab);
          return;
        }
        if (tab?.id !== tabId) return;
        if (!pausable) {
          show(state);
          return;
        }
        if (state === "recording") {
          show("pausing");
          port.postMessage({ type: "meeting_pause" });
        } else if (state === "paused") {
          show("resuming");
          port.postMessage({ type: "meeting_resume" });
        }
      },
      // The call ended in the tab: Anarlog stops a recording the tab started and keeps one its own
      // window started, which only the person stops.
      ended: (sender) => {
        if (sender?.tab?.id === tabId && meetingUrl(sender.url) === url) leave();
      },
      // What Meet shows about who is speaking, from the tab being recorded only.
      speakers: (sender, events) => {
        if (!port || state === "stopping" || !Array.isArray(events)) return;
        if (sender?.tab?.id === tabId && meetingUrl(sender.url) === url)
          port.postMessage({ type: "meeting_cues", events });
      },
      active: () => port !== null,
      removed: (id) => {
        if (id === tabId) leave();
      },
    };
  }

  global.AnarlogMeetingBackground = { MENU, CONNECT_TIMEOUT_MS, meetingUrl, createController };
  const api = global.chrome;
  if (!api || !api.contextMenus || !global.AnarlogMeetingConfig) return;
  const controller = createController(api, global.AnarlogMeetingConfig.nativeHost);
  global.AnarlogMeetingController = controller;
  api.contextMenus.onClicked.addListener(controller.click);
  api.tabs.onUpdated.addListener(controller.updated);
  api.tabs.onRemoved.addListener(controller.removed);
  api.runtime.onMessage.addListener((message, sender) => {
    if (message?.type === "anarlog_meeting_ended") controller.ended(sender);
    if (message?.type === "anarlog_meeting_speakers") controller.speakers(sender, message.events);
  });
})(globalThis);
