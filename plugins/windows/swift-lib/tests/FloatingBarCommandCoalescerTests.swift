import XCTest

@testable import swift_lib

@_cdecl("rust_on_floating_bar_stop")
func rustOnFloatingBarStop() {}

@_cdecl("rust_on_floating_bar_open_main")
func rustOnFloatingBarOpenMain() {}

@_cdecl("rust_on_floating_bar_settings_change")
func rustOnFloatingBarSettingsChange(_: UnsafePointer<CChar>) {}

@_cdecl("rust_on_devtools_panel_action")
func rustOnDevtoolsPanelAction(_: UnsafePointer<CChar>) {}

final class FloatingBarCommandCoalescerTests: XCTestCase {
  func testCoalescesBurstAndPreservesLatestNonNilTranscript() {
    let scheduler = ManualScheduler()
    var actions: [FloatingBarCommandCoalescer.Action] = []
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { actions.append($0) })

    coalescer.enqueueUpdate(state(amplitude: 0, bubbles: [bubble(id: "first")]))
    for index in 1...10_000 {
      coalescer.enqueueUpdate(state(amplitude: Double(index), bubbles: nil))
    }

    XCTAssertEqual(scheduler.pendingCount, 1)
    scheduler.runNext()

    XCTAssertEqual(actions.count, 1)
    guard case .update(let state) = actions.first else {
      return XCTFail("Expected a coalesced update")
    }
    XCTAssertEqual(state.amplitude, 10_000)
    XCTAssertEqual(state.transcriptBubbles?.map(\.id), ["first"])
  }

  func testNewestNonNilTranscriptWinsAndEmptyArrayClears() {
    let scheduler = ManualScheduler()
    var actions: [FloatingBarCommandCoalescer.Action] = []
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { actions.append($0) })

    coalescer.enqueueUpdate(state(amplitude: 1, bubbles: [bubble(id: "first")]))
    coalescer.enqueueUpdate(state(amplitude: 2, bubbles: [bubble(id: "second")]))
    coalescer.enqueueUpdate(state(amplitude: 3, bubbles: nil))
    scheduler.runNext()

    guard case .update(let latestState) = actions.last else {
      return XCTFail("Expected an update")
    }
    XCTAssertEqual(latestState.transcriptBubbles?.map(\.id), ["second"])

    coalescer.enqueueUpdate(state(amplitude: 4, bubbles: [bubble(id: "old")]))
    coalescer.enqueueUpdate(state(amplitude: 5, bubbles: []))
    coalescer.enqueueUpdate(state(amplitude: 6, bubbles: nil))
    scheduler.runNext()

    guard case .update(let clearedState) = actions.last else {
      return XCTFail("Expected an update")
    }
    XCTAssertEqual(clearedState.transcriptBubbles?.count, 0)
  }

  func testAppliesLatestVisibilityAndUpdateInCallOrder() {
    let scheduler = ManualScheduler()
    var actionNames: [String] = []
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { action in
        switch action {
        case .show: actionNames.append("show")
        case .hide: actionNames.append("hide")
        case .update: actionNames.append("update")
        }
      })

    coalescer.enqueueShow()
    coalescer.enqueueUpdate(state(amplitude: 1, bubbles: nil))
    coalescer.enqueueHide()
    scheduler.runNext()
    XCTAssertEqual(actionNames, ["update", "hide"])

    coalescer.enqueueHide()
    coalescer.enqueueUpdate(state(amplitude: 2, bubbles: nil))
    coalescer.enqueueShow()
    scheduler.runNext()
    XCTAssertEqual(actionNames, ["update", "hide", "update", "show"])
  }

  func testSchedulesOneFollowUpWhenWorkArrivesDuringApply() {
    let scheduler = ManualScheduler()
    var amplitudes: [Double] = []
    var coalescer: FloatingBarCommandCoalescer!
    coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { action in
        guard case .update(let state) = action else { return }
        amplitudes.append(state.amplitude)
        if state.amplitude == 1 {
          coalescer.enqueueUpdate(self.state(amplitude: 2, bubbles: nil))
          coalescer.enqueueUpdate(self.state(amplitude: 3, bubbles: nil))
        }
      })

    coalescer.enqueueUpdate(state(amplitude: 1, bubbles: nil))
    scheduler.runNext()

    XCTAssertEqual(amplitudes, [1])
    XCTAssertEqual(scheduler.pendingCount, 1)
    scheduler.runNext()
    XCTAssertEqual(amplitudes, [1, 3])
    XCTAssertEqual(scheduler.pendingCount, 0)
  }

  func testConcurrentIngressStillSchedulesOneLatestStateDrain() {
    let scheduler = ManualScheduler()
    var actions: [FloatingBarCommandCoalescer.Action] = []
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { actions.append($0) })
    let producers = DispatchGroup()

    for producer in 0..<8 {
      producers.enter()
      DispatchQueue.global().async {
        for index in 0..<1_000 {
          coalescer.enqueueUpdate(
            self.state(
              amplitude: Double(producer * 1_000 + index),
              bubbles: nil))
        }
        producers.leave()
      }
    }

    XCTAssertEqual(producers.wait(timeout: .now() + 5), .success)
    coalescer.enqueueUpdate(
      state(amplitude: 99_999, bubbles: [bubble(id: "final")]))
    coalescer.enqueueShow()

    XCTAssertEqual(scheduler.pendingCount, 1)
    scheduler.runNext()

    XCTAssertEqual(actions.count, 2)
    guard case .update(let finalState) = actions.first else {
      return XCTFail("Expected the deterministic final update first")
    }
    XCTAssertEqual(finalState.amplitude, 99_999)
    XCTAssertEqual(finalState.transcriptBubbles?.map(\.id), ["final"])
    guard case .show = actions.last else {
      return XCTFail("Expected the final visibility command")
    }
  }

  private func state(
    amplitude: Double,
    bubbles: [FloatingTranscriptBubblePayload]?
  ) -> FloatingBarStatePayload {
    FloatingBarStatePayload(
      amplitude: amplitude,
      title: "Meeting",
      status: .recording,
      colorScheme: .dark,
      opacity: 0.8,
      liveCaptionOpacity: 0.3,
      liveCaptionWidth: 440,
      liveCaptionLineCount: 1,
      liveCaptionPosition: .topCenter,
      liveCaptionMinimized: true,
      liveCaptionToggleVisible: true,
      transcriptBubbles: bubbles)
  }

  private func bubble(id: String) -> FloatingTranscriptBubblePayload {
    FloatingTranscriptBubblePayload(
      id: id,
      speakerLabel: "You",
      text: id,
      isSelf: true,
      isFinal: true,
      startMs: 0,
      endMs: 1,
      overlapsPrevious: false,
      overlapsNext: false)
  }
}

private final class ManualScheduler {
  private let lock = NSLock()
  private var work: [() -> Void] = []

  var pendingCount: Int {
    lock.lock()
    defer { lock.unlock() }
    return work.count
  }

  func schedule(_ next: @escaping () -> Void) {
    lock.lock()
    work.append(next)
    lock.unlock()
  }

  func runNext() {
    lock.lock()
    let next = work.removeFirst()
    lock.unlock()
    next()
  }
}
