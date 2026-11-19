(function () {
  // Another recording in a tab already watching: watch again from here.
  if (typeof globalThis.anarlogMeetingMonitor?.restart === "function") {
    globalThis.anarlogMeetingMonitor.restart();
    return;
  }
  const ended = () => {
    void chrome.runtime
      .sendMessage({ type: "anarlog_meeting_ended" })
      .catch(() => {});
  };
  // Positive end cues only. Missing controls (loading, PiP, or hidden tabs) do not
  // imply the call ended. Unknown languages retain navigation/close/manual Stop.
  const leaveClicked = (event) => {
    const label = event.target.closest?.("button")?.getAttribute("aria-label");
    if (
      ["Leave call", "Leave the call", "통화 종료", "통화에서 나가기"].includes(
        label,
      )
    )
      ended();
  };
  let timer;
  const check = () => {
    const titles = [...document.querySelectorAll('h1,h2,[role="heading"]')];
    if (
      titles.some((node) =>
        /^(you('ve| have)? left the (meeting|call)|you left the (meeting|call)|the meeting has ended|회의에서 나왔습니다|통화가 종료되었습니다)[.!]?$/i.test(
          node.textContent.trim(),
        ),
      )
    )
      ended();
  };
  // At most one check per 150 ms. A live Meet page mutates many times a second, so waiting for
  // the DOM to go quiet would never check at all.
  const observer = new MutationObserver(() => {
    if (timer) return;
    timer = setTimeout(() => {
      timer = undefined;
      check();
    }, 150);
  });
  /** Stops watching, as when the tab lets go of the recording; safe to call again. */
  const stop = () => {
    observer.disconnect();
    clearTimeout(timer);
    timer = undefined;
    document.removeEventListener("click", leaveClicked, true);
  };
  const restart = () => {
    stop();
    document.addEventListener("click", leaveClicked, true);
    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    check();
  };
  globalThis.anarlogMeetingMonitor = { stop, restart };
  restart();
})();
