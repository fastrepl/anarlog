import Darwin
import XCTest

@testable import swift_lib

@_cdecl("rust_on_floating_bar_stop")
func rustOnFloatingBarStop() {}

@_cdecl("rust_on_floating_bar_open_main")
func rustOnFloatingBarOpenMain() {}

@_cdecl("rust_on_floating_bar_settings_change")
func rustOnFloatingBarSettingsChange(_: UnsafePointer<CChar>) {}

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

  func testCoalescesAmplitudeOnlyUpdatesWithoutBuildingFullState() {
    let scheduler = ManualScheduler()
    var actions: [FloatingBarCommandCoalescer.Action] = []
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { actions.append($0) })

    coalescer.enqueueAmplitude(0.1)
    coalescer.enqueueAmplitude(0.2)
    coalescer.enqueueAmplitude(0.3)
    scheduler.runNext()

    XCTAssertEqual(actions.count, 1)
    guard case .amplitude(let amplitude) = actions.first else {
      return XCTFail("Expected an amplitude-only update")
    }
    XCTAssertEqual(amplitude, 0.3)
  }

  func testAmplitudeUpdateMergesIntoPendingFullState() {
    let scheduler = ManualScheduler()
    var actions: [FloatingBarCommandCoalescer.Action] = []
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { actions.append($0) })

    coalescer.enqueueUpdate(state(amplitude: 0.1, bubbles: [bubble(id: "latest")]))
    coalescer.enqueueAmplitude(0.8)
    scheduler.runNext()

    XCTAssertEqual(actions.count, 1)
    guard case .update(let state) = actions.first else {
      return XCTFail("Expected the full update to be preserved")
    }
    XCTAssertEqual(state.amplitude, 0.8)
    XCTAssertEqual(state.transcriptBubbles?.map(\.id), ["latest"])
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
        case .amplitude: actionNames.append("amplitude")
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

  func testPendingDrainDoesNotRetainCoalescerAfterTeardown() {
    let scheduler = ManualScheduler()
    var coalescer: FloatingBarCommandCoalescer? = FloatingBarCommandCoalescer(
      scheduler: scheduler.schedule,
      apply: { _ in XCTFail("A torn-down coalescer must not apply pending work") })
    let weakCoalescer = WeakReference(coalescer!)

    coalescer?.enqueueAmplitude(0.5)
    XCTAssertEqual(scheduler.pendingCount, 1)

    coalescer = nil
    XCTAssertNil(weakCoalescer.value)
    scheduler.runNext()
  }

  func testRecordingRateAmplitudeBridgeSoakHasBoundedResidentMemory() async throws {
    let environment = ProcessInfo.processInfo.environment
    guard environment["ANLG_RUN_BRIDGE_SOAK"] == "1" else {
      throw XCTSkip("Set ANLG_RUN_BRIDGE_SOAK=1 to run the 30-minute bridge soak")
    }

    let duration = TimeInterval(environment["ANLG_BRIDGE_SOAK_SECONDS"] ?? "1800") ?? 1800
    let sampleInterval = TimeInterval(environment["ANLG_BRIDGE_SOAK_SAMPLE_SECONDS"] ?? "60") ?? 60
    let samples = ResidentMemorySamples()
    let finished = expectation(description: "Amplitude bridge soak finished")

    await MainActor.run {
      XCTAssertTrue(_floatingBarUpdateAmplitude(amplitude: 0))
    }

    DispatchQueue.global(qos: .userInitiated).async {
      let start = Date()
      var nextSample = 0.0
      var tick = 0

      while true {
        let elapsed = Date().timeIntervalSince(start)
        guard elapsed < duration else { break }

        XCTAssertTrue(
          _floatingBarUpdateAmplitude(amplitude: Double(tick % 101) / 100))
        tick += 1

        if elapsed >= nextSample {
          samples.append(elapsed: elapsed, residentBytes: residentMemoryBytes())
          nextSample += sampleInterval
        }

        Thread.sleep(forTimeInterval: 0.1)
      }

      samples.append(
        elapsed: Date().timeIntervalSince(start),
        residentBytes: residentMemoryBytes())
      finished.fulfill()
    }

    await fulfillment(of: [finished], timeout: duration + 30)

    let measured = samples.values
    XCTAssertGreaterThanOrEqual(measured.count, 3)
    let warmup = min(60, duration * 0.1)
    let postWarmup = measured.filter { $0.elapsed >= warmup }
    XCTAssertGreaterThanOrEqual(postWarmup.count, 2)

    let growth = Int64(postWarmup.last!.residentBytes) - Int64(postWarmup.first!.residentBytes)
    let slope = residentMemorySlopeBytesPerMinute(postWarmup)
    let mib = 1024.0 * 1024.0

    print(
      "bridge soak samples=\(measured.count) "
        + "growth_mib=\(Double(growth) / mib) "
        + "slope_mib_per_minute=\(slope / mib)")
    XCTAssertLessThan(growth, 32 * 1024 * 1024)
    XCTAssertLessThan(slope, 2 * 1024 * 1024)
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

private struct ResidentMemorySample {
  let elapsed: TimeInterval
  let residentBytes: UInt64
}

private final class WeakReference<Value: AnyObject> {
  weak var value: Value?

  init(_ value: Value) {
    self.value = value
  }
}

private final class ResidentMemorySamples: @unchecked Sendable {
  private let lock = NSLock()
  private var samples: [ResidentMemorySample] = []

  var values: [ResidentMemorySample] {
    lock.lock()
    defer { lock.unlock() }
    return samples
  }

  func append(elapsed: TimeInterval, residentBytes: UInt64) {
    lock.lock()
    samples.append(
      ResidentMemorySample(elapsed: elapsed, residentBytes: residentBytes))
    lock.unlock()
  }
}

private func residentMemoryBytes() -> UInt64 {
  var info = proc_taskinfo()
  let size = MemoryLayout<proc_taskinfo>.stride
  let result = withUnsafeMutablePointer(to: &info) { pointer in
    proc_pidinfo(getpid(), PROC_PIDTASKINFO, 0, pointer, Int32(size))
  }
  return result == Int32(size) ? info.pti_resident_size : 0
}

private func residentMemorySlopeBytesPerMinute(
  _ samples: [ResidentMemorySample]
) -> Double {
  let meanElapsed = samples.map(\.elapsed).reduce(0, +) / Double(samples.count)
  let meanResident =
    samples.map { Double($0.residentBytes) }.reduce(0, +)
    / Double(samples.count)
  let covariance = samples.reduce(0.0) { partial, sample in
    partial
      + (sample.elapsed - meanElapsed) * (Double(sample.residentBytes) - meanResident)
  }
  let variance = samples.reduce(0.0) { partial, sample in
    partial + pow(sample.elapsed - meanElapsed, 2)
  }
  guard variance > 0 else { return 0 }
  return covariance / variance * 60
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
