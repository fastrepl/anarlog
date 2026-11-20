import AVFoundation
import CoreAudio
import XCTest

@testable import CaptureCore

final class CallAudioScopeTests: XCTestCase {
  let chrome = CallAudioScope.application(pid: 10, bundleID: "com.google.Chrome")

  // Chrome plays a Meet call from its audio helper, so the helper is tapped with Chrome.
  func testAnAppIsTappedWithItsHelpers() {
    let processes = [
      AudioProcess(object: 100, pid: 10, bundleID: "com.google.Chrome"),
      AudioProcess(object: 101, pid: 11, bundleID: "com.google.Chrome.helper"),
      AudioProcess(object: 200, pid: 20, bundleID: "com.spotify.client"),
      AudioProcess(object: 300, pid: 30, bundleID: "com.tinyspeck.slackmacgap"),
    ]
    let parents: [Int32: Int32] = [10: 1, 11: 10, 20: 1, 30: 1]
    XCTAssertEqual(
      tapPlan(chrome, processes: processes, parent: { parents[$0] }),
      TapPlan(excluding: false, processes: [100, 101]))
  }

  // Chrome Beta, and a second Chrome with its own profile, were started by launchd.
  func testASeparateBrowserStaysOut() {
    let processes = [
      AudioProcess(object: 100, pid: 10, bundleID: "com.google.Chrome"),
      AudioProcess(object: 101, pid: 11, bundleID: "com.google.Chrome.helper"),
      AudioProcess(object: 400, pid: 40, bundleID: "com.google.Chrome.beta"),
      AudioProcess(object: 401, pid: 41, bundleID: "com.google.Chrome.beta.helper"),
      AudioProcess(object: 500, pid: 50, bundleID: "com.google.Chrome"),
      AudioProcess(object: 501, pid: 51, bundleID: "com.google.Chrome.helper"),
    ]
    let parents: [Int32: Int32] = [10: 1, 11: 10, 40: 1, 41: 40, 50: 1, 51: 50]
    XCTAssertEqual(
      tapPlan(chrome, processes: processes, parent: { parents[$0] }).processes, [100, 101])
  }

  // A helper started by a helper (Chrome's audio service under its GPU process) still counts.
  func testAHelperOfAHelperCounts() {
    let processes = [AudioProcess(object: 102, pid: 12, bundleID: "com.google.Chrome.helper")]
    let parents: [Int32: Int32] = [12: 11, 11: 10, 10: 1]
    XCTAssertEqual(
      tapPlan(chrome, processes: processes, parent: { parents[$0] }).processes, [102])
  }

  // Dia's helpers are company.thebrowser.browser.helper, not a prefix of Dia's bundle ID.
  func testAHelperWithAnotherBundleIDCounts() {
    let dia = CallAudioScope.application(pid: 10, bundleID: "company.thebrowser.dia")
    let processes = [
      AudioProcess(object: 101, pid: 11, bundleID: "company.thebrowser.browser.helper")
    ]
    XCTAssertEqual(
      tapPlan(dia, processes: processes, parent: { $0 == 11 ? 10 : 1 }).processes, [101])
  }

  func testAnAppWithoutABundleIDTapsItsOwnTree() {
    let processes = [
      AudioProcess(object: 100, pid: 10, bundleID: ""),
      AudioProcess(object: 200, pid: 20, bundleID: ""),
    ]
    XCTAssertEqual(
      tapPlan(.application(pid: 10, bundleID: ""), processes: processes, parent: { _ in 1 })
        .processes, [100])
  }

  func testTheWholeMacLeavesOutOnlyTheRecorder() {
    let processes = [
      AudioProcess(object: 100, pid: 10, bundleID: "com.google.Chrome"),
      AudioProcess(object: 700, pid: 70, bundleID: "so.anarlog.meeting-capture"),
    ]
    XCTAssertEqual(
      tapPlan(.system, processes: processes, recorder: 70),
      TapPlan(excluding: true, processes: [700]))
  }

  func testAnAppIsTappedAsAMonoMixOfItsProcesses() {
    let uuid = UUID()
    let description = tapDescription(TapPlan(excluding: false, processes: [100, 101]), uuid: uuid)
    XCTAssertFalse(description.isExclusive)
    XCTAssertEqual(description.processes, [100, 101])
    XCTAssertTrue(description.isMono)
    XCTAssertTrue(description.isPrivate)
    XCTAssertEqual(description.muteBehavior, .unmuted)
    XCTAssertEqual(description.uuid, uuid)
  }

  func testTheWholeMacIsAGlobalTapThatExcludes() {
    let description = tapDescription(TapPlan(excluding: true, processes: [700]), uuid: UUID())
    XCTAssertTrue(description.isExclusive)
    XCTAssertEqual(description.processes, [700])
    XCTAssertTrue(description.isMono)
  }

  func testTheRealParentLookupFindsThisProcess() {
    XCTAssertEqual(parentProcessID(getpid()), getppid())
    XCTAssertTrue(descends(getpid(), from: getppid(), parent: parentProcessID))
  }
}

/// Device changes rebuild the tap and say so in the journal; nothing ends the recording.
@MainActor
final class CallAudioRecoveryTests: XCTestCase {
  final class FakeBackend {
    var processes: [AudioProcess] = [AudioProcess(object: 100, pid: 10, bundleID: "com.example")]
    var opened: [TapPlan] = []
    var attempts = 0
    var closed = 0
    var updates: [TapPlan] = []
    var failNextOpens = 0
    var acceptsUpdates = true
    var changed: (@MainActor (CallAudioChange) -> Void)?
    var deliver: ((AVAudioPCMBuffer, Double) -> Void)?

    var backend: CallAudioBackend {
      CallAudioBackend(
        processes: { self.processes },
        open: { plan, deliver, _ in
          self.attempts += 1
          if self.failNextOpens > 0 {
            self.failNextOpens -= 1
            throw CaptureError.invalid("No output device")
          }
          self.opened.append(plan)
          self.deliver = deliver
          return CallAudioSession(
            update: { plan in
              self.updates.append(plan)
              return self.acceptsUpdates
            }, close: { self.closed += 1 })
        },
        watch: { changed in
          self.changed = changed
          return {}
        })
    }
  }

  var directory: URL!

  override func setUp() async throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
  }

  override func tearDown() async throws {
    try? FileManager.default.removeItem(at: directory)
  }

  func events() throws -> [String] {
    try String(contentsOf: directory.appendingPathComponent("capture.journal"), encoding: .utf8)
      .split(separator: "\n").compactMap { EventLine.parse(String($0))?.event }
  }

  func until(_ condition: @escaping () -> Bool) async throws {
    for _ in 0..<100 {
      if condition() { return }
      try await Task.sleep(for: .milliseconds(20))
    }
    XCTFail("Timed out")
  }

  func capture(
    _ fake: FakeBackend, scope: CallAudioScope = .application(pid: 10, bundleID: "com.example")
  ) throws -> (CallAudio, AudioOutput) {
    let output = try AudioOutput(
      directory: directory, captureID: "capture", processID: 10, bundleID: "com.example",
      microphoneID: nil)
    let audio = CallAudio(scope: scope, output: output, backend: fake.backend)
    audio.retryDelay = .milliseconds(20)
    audio.stallSeconds = 3600
    return (audio, output)
  }

  func testANewOutputDeviceRebuildsTheTap() async throws {
    let fake = FakeBackend()
    let (audio, output) = try capture(fake)
    audio.start()
    fake.changed?(.output)
    try await until { fake.opened.count == 2 }
    audio.stop()
    XCTAssertEqual(fake.closed, 2)
    _ = try output.finish(reason: "user_stopped")
    XCTAssertEqual(
      try events(),
      [
        "prepared", "call-audio-configured", "call-audio-interrupted", "call-audio-configured",
        "ended",
      ])
  }

  func testATapThatCannotReopenRetriesUntilTheDeviceReturns() async throws {
    let fake = FakeBackend()
    let (audio, output) = try capture(fake)
    audio.start()
    fake.failNextOpens = 3
    fake.changed?(.output)
    try await until { fake.opened.count == 2 }
    audio.stop()
    _ = try output.finish(reason: "user_stopped")
    // The failure is noted once, not on every retry.
    XCTAssertEqual(
      try events(),
      [
        "prepared", "call-audio-configured", "call-audio-interrupted", "call-audio-unavailable",
        "call-audio-configured", "ended",
      ])
  }

  func testNoTapAtStartStillStartsTheRecording() async throws {
    let fake = FakeBackend()
    fake.failNextOpens = 2
    let (audio, output) = try capture(fake)
    audio.start()
    try await until { fake.opened.count == 1 }
    audio.stop()
    _ = try output.finish(reason: "user_stopped")
    XCTAssertEqual(
      try events(), ["prepared", "call-audio-unavailable", "call-audio-configured", "ended"])
  }

  /// A Mac with no output device, or a tap macOS keeps refusing: the microphone is already
  /// recording, and the recorder reads Stop only once the start returns. So the start never waits
  /// for the tap, and Stop ends a recording whose call track stayed silent.
  func testATapThatNeverOpensDoesNotHoldUpTheStart() async throws {
    let fake = FakeBackend()
    fake.failNextOpens = .max
    let (audio, output) = try capture(fake)
    audio.start()
    XCTAssertEqual(fake.attempts, 1)
    try await until { fake.attempts >= 3 }
    audio.stop()
    let stopped = fake.attempts
    try await Task.sleep(for: .milliseconds(400))
    XCTAssertEqual(fake.attempts, stopped, "it kept retrying after Stop")
    _ = try output.finish(reason: "user_stopped")
    XCTAssertEqual(try events(), ["prepared", "call-audio-unavailable", "ended"])
  }

  // This test process stands in for a helper its parent started after the recording began.
  func testAHelperThatStartsLaterJoinsTheRunningTap() async throws {
    let fake = FakeBackend()
    let app = getppid()
    fake.processes = [AudioProcess(object: 100, pid: app, bundleID: "com.example")]
    let (audio, output) = try capture(fake, scope: .application(pid: app, bundleID: "com.example"))
    audio.start()
    fake.processes += [
      AudioProcess(object: 101, pid: getpid(), bundleID: "com.example.helper"),
      AudioProcess(object: 200, pid: 1, bundleID: "com.other"),
    ]
    fake.changed?(.processes)
    XCTAssertEqual(fake.updates, [TapPlan(excluding: false, processes: [100, 101])])
    // A list change that leaves the plan as it was does nothing.
    fake.changed?(.processes)
    XCTAssertEqual(fake.updates.count, 1)
    XCTAssertEqual(fake.opened.count, 1)
    audio.stop()
    _ = try output.finish(reason: "user_stopped")
    XCTAssertEqual(
      try events(),
      ["prepared", "call-audio-configured", "call-audio-processes-changed", "ended"])
  }

  func testATapThatRefusesItsNewProcessesIsRebuilt() async throws {
    let fake = FakeBackend()
    fake.acceptsUpdates = false
    let (audio, output) = try capture(fake)
    audio.start()
    fake.processes = [AudioProcess(object: 102, pid: 10, bundleID: "com.example")]
    fake.changed?(.processes)
    try await until { fake.opened.count == 2 }
    XCTAssertEqual(fake.opened.last?.processes, [102])
    audio.stop()
    _ = try output.finish(reason: "user_stopped")
  }

  func testATapThatGoesQuietWhileItsAppPlaysIsRebuilt() async throws {
    let fake = FakeBackend()
    fake.processes[0].isPlaying = true
    let (audio, output) = try capture(fake)
    audio.stallSeconds = 0.2
    audio.start()
    fake.deliver?(silence(), captureHostTime())
    try await until { fake.opened.count == 2 }
    audio.stop()
    _ = try output.finish(reason: "user_stopped")
    XCTAssertTrue(try events().contains("call-audio-interrupted"))
  }

  // A tap gets no buffers while its app is silent. That is not a stall.
  func testASilentAppIsNotAStall() async throws {
    let fake = FakeBackend()
    let (audio, output) = try capture(fake)
    audio.stallSeconds = 0.1
    audio.start()
    fake.deliver?(silence(), captureHostTime())
    try await Task.sleep(for: .milliseconds(1200))
    // Nor is a tap that never heard anything since it opened, even with its app playing.
    fake.processes[0].isPlaying = true
    fake.changed?(.output)
    try await until { fake.opened.count == 2 }
    try await Task.sleep(for: .milliseconds(1200))
    XCTAssertEqual(fake.opened.count, 2)
    audio.stop()
    _ = try output.finish(reason: "user_stopped")
  }

  func testWhatATapShouldHear() {
    let processes = [
      AudioProcess(object: 100, pid: 10, bundleID: "a", isPlaying: false),
      AudioProcess(object: 200, pid: 20, bundleID: "b", isPlaying: true),
    ]
    XCTAssertFalse(expectsAudio(TapPlan(excluding: false, processes: [100]), processes: processes))
    XCTAssertTrue(expectsAudio(TapPlan(excluding: false, processes: [200]), processes: processes))
    XCTAssertTrue(expectsAudio(TapPlan(excluding: true, processes: [100]), processes: processes))
    XCTAssertFalse(expectsAudio(TapPlan(excluding: true, processes: [200]), processes: processes))
  }

  func silence() -> AVAudioPCMBuffer {
    let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 480)!
    buffer.frameLength = 480
    buffer.floatChannelData![0].initialize(repeating: 0, count: 480)
    return buffer
  }

  /// A full disk is not a bad buffer from a device mid-change: the recording stops and says
  /// why, instead of going on with a call track it can no longer write.
  func testAFullDiskEndsTheRecording() throws {
    let output = try AudioOutput(
      directory: directory, captureID: "capture", processID: 10, bundleID: "com.example",
      microphoneID: nil)
    try output.started(EventLine("started"))
    // No file may grow past 64 KB, as on a full disk: about a third of a second of the call.
    let previousSignal = signal(SIGXFSZ, SIG_IGN)
    var previous = rlimit()
    getrlimit(RLIMIT_FSIZE, &previous)
    var limit = previous
    limit.rlim_cur = 64 * 1024
    setrlimit(RLIMIT_FSIZE, &limit)
    defer {
      setrlimit(RLIMIT_FSIZE, &previous)
      signal(SIGXFSZ, previousSignal)
    }
    let start = captureHostTime()
    for second in 0..<10 {
      output.appendRemote(tone(), hostTime: start + Double(second) * 0.1)
    }
    try output.end()
    XCTAssertTrue(output.error is AudioWriteError, "\(String(describing: output.error))")
  }

  func tone() -> AVAudioPCMBuffer {
    let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4800)!
    buffer.frameLength = 4800
    buffer.floatChannelData![0].initialize(repeating: 0.25, count: 4800)
    return buffer
  }

  func testCallBuffersReachTheRemoteTrack() async throws {
    let fake = FakeBackend()
    let (audio, output) = try capture(fake)
    audio.start()
    try output.started(EventLine("started"))
    let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4800)!
    buffer.frameLength = 4800
    for i in 0..<4800 { buffer.floatChannelData![0][i] = 0.25 }
    fake.deliver?(buffer, captureHostTime())
    audio.stop()
    try output.end()
    let result = try output.finish(reason: "user_stopped")
    XCTAssertGreaterThan(Int64(result.value("remoteFrames") ?? "") ?? 0, 0)
    XCTAssertEqual(Double(result.value("remotePeak") ?? "") ?? 0, 0.25, accuracy: 0.01)
  }
}
