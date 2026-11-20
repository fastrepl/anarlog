import AVFoundation
import XCTest

@testable import CaptureCore

/// Device QA found a plain microphone that went silent, or recorded a new default input under the
/// old name, with no configuration change from AVAudioEngine. It must rebuild on the Mac's
/// current default input and say so; nothing ends the recording.
@MainActor
final class MicrophoneRecoveryTests: XCTestCase {
  final class FakeMicrophone {
    var defaultInput = InputDevice(
      id: 1, uid: "BuiltInMicrophoneDevice", name: "MacBook Pro Microphone")
    var route: OutputRoute?
    /// Where AVAudioEngine left the input unit; nil while it follows the default input at once.
    var bound: InputBinding?
    var rate = 48_000.0
    /// The input each graph opened on, oldest first.
    var opened: [InputDevice] = []
    /// Where the unit was as each graph opened.
    var openedOn: [InputBinding] = []
    /// The devices the helper bound the unit to itself.
    var binds: [AudioObjectID] = []
    var taps: [@Sendable (AVAudioPCMBuffer, Double) -> Void] = []
    /// Graphs, by index into `opened`, that run but never deliver a buffer.
    var dead: Set<Int> = []
    /// The replaced graph's tap calls back once more just after the next one starts.
    var lateCallbacks = false
    /// Runs once, as the next graph starts.
    var onStart: (() -> Void)?
    var onBindingRead: (() -> Void)?
    var running = false
    var changed: (@MainActor (MicrophoneChange) -> Void)?

    var binding: InputBinding {
      bound ?? InputBinding(device: defaultInput.id, inputs: [defaultInput.id], sampleRate: rate)
    }

    var backend: MicrophoneBackend {
      MicrophoneBackend(
        defaultInput: { self.defaultInput },
        outputRoute: { self.route },
        binding: {
          let binding = self.binding
          let onRead = self.onBindingRead
          self.onBindingRead = nil
          onRead?()
          return binding
        },
        open: { _, deliver in
          self.opened.append(self.defaultInput)
          self.openedOn.append(self.binding)
          let device = try followInput(self.defaultInput, binding: { self.binding }) {
            self.binds.append($0)
            self.bound = InputBinding(device: $0, inputs: [$0], sampleRate: self.rate)
          }
          self.taps.append(deliver)
          return MicrophoneGraph(
            format: AVAudioFormat(
              standardFormatWithSampleRate: self.binding.sampleRate, channels: 1)!,
            device: device)
        },
        start: {
          self.running = true
          if self.lateCallbacks, self.taps.count > 1 {
            let replaced = self.taps[self.taps.count - 2]
            Task { @MainActor in
              try? await Task.sleep(for: .milliseconds(10))
              replaced(silence(), captureHostTime())
            }
          }
          let onStart = self.onStart
          self.onStart = nil
          onStart?()
        },
        isRunning: { self.running },
        close: { self.running = false },
        voiceProcessingSettings: { (10, false, false) },
        watch: { changed in
          self.changed = changed
          return {}
        })
    }

    /// One buffer of digital silence from the newest graph, as its running tap delivers in a
    /// quiet room.
    func tick() {
      guard running, let tap = taps.last, !dead.contains(taps.count - 1) else { return }
      tap(silence(), captureHostTime())
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

  func capture(_ fake: FakeMicrophone, processing: MicrophoneProcessing = .plain) throws
    -> (VoiceMicrophone, AudioOutput, Task<Void, Never>)
  {
    let output = try AudioOutput(
      directory: directory, captureID: "capture", processID: 0, bundleID: "", microphoneID: nil)
    let microphone = VoiceMicrophone(processing: processing, backend: fake.backend)
    microphone.settleTime = .milliseconds(50)
    microphone.stallSeconds = 0.5
    let feed = Task { @MainActor in
      while !Task.isCancelled {
        fake.tick()
        try? await Task.sleep(for: .milliseconds(10))
      }
    }
    return (microphone, output, feed)
  }

  func journal() throws -> [EventLine] {
    try String(contentsOf: directory.appendingPathComponent("capture.journal"), encoding: .utf8)
      .split(separator: "\n").compactMap { EventLine.parse(String($0)) }
  }

  func until(_ condition: @escaping () -> Bool) async throws {
    for _ in 0..<250 {
      if condition() { return }
      try await Task.sleep(for: .milliseconds(20))
    }
    XCTFail("Timed out")
  }

  func recoveries() -> Int {
    ((try? journal()) ?? []).filter { $0.event == "microphone-recovered" }.count
  }

  func testAnOutputChangeDuringTheInputWaitEnablesVoiceProcessingOnSpeakers() async throws {
    let fake = FakeMicrophone()
    let builtIn = fake.defaultInput
    fake.defaultInput = usb
    fake.bound = InputBinding(device: usb.id, inputs: [usb.id], sampleRate: 48_000)
    fake.route = OutputRoute(
      name: "AirPods Pro", uid: "headphones", transport: kAudioDeviceTransportTypeBluetooth,
      dataSource: nil)
    let speakers = OutputRoute(
      name: "MacBook Pro Speakers", uid: "speakers", transport: kAudioDeviceTransportTypeBuiltIn,
      dataSource: nil)
    let (microphone, output, feed) = try capture(fake, processing: .automatic)
    defer {
      microphone.stop()
      feed.cancel()
    }
    try await microphone.start(deviceUID: usb.uid, output: output)
    XCTAssertEqual(microphone.usesVoiceProcessing, false)

    fake.defaultInput = builtIn
    fake.onBindingRead = {
      Task { @MainActor in
        fake.route = speakers
        fake.changed?(.output)
        fake.bound = nil
      }
    }
    fake.changed?(.input)
    try await until { self.recoveries() == 1 }
    XCTAssertEqual(microphone.usesVoiceProcessing, true)
    microphone.stop()
    _ = try output.finish(reason: "user_stopped")
    let configured = try journal().last { $0.event == "microphone-configured" }
    XCTAssertEqual(configured?.value("voiceProcessing"), "true")
    XCTAssertEqual(configured?.value("output"), speakers.name)
  }

  /// As on device QA: AVAudioEngine reported built-in → USB, but not USB → built-in.
  func testAPlainMicrophoneFollowsANewDefaultInputTheEngineDoesNotReport() async throws {
    let fake = FakeMicrophone()
    let builtIn = fake.defaultInput
    let (microphone, output, feed) = try capture(fake)
    defer { feed.cancel() }
    try await microphone.start(deviceUID: builtIn.uid, output: output)
    // The engine's report lands with the input's, and joins its one rebuild. It reports again as
    // the rebuilt graph starts on the new input, which the graph outlasts.
    fake.defaultInput = usb
    fake.onStart = { fake.changed?(.configuration) }
    fake.changed?(.input)
    fake.changed?(.configuration)
    try await until { self.recoveries() == 1 }
    fake.defaultInput = builtIn
    fake.changed?(.input)
    try await until { self.recoveries() == 2 }
    microphone.stop()
    _ = try output.finish(reason: "user_stopped")
    let lines = try journal()
    XCTAssertEqual(
      lines.map(\.event),
      [
        "prepared", "microphone-configured", "microphone-interrupted", "microphone-configured",
        "microphone-recovered", "microphone-interrupted", "microphone-configured",
        "microphone-recovered", "ended",
      ])
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-interrupted" }.map { $0.value("reason") },
      ["input-changed", "input-changed"])
    XCTAssertEqual(fake.opened, [builtIn, usb, builtIn])
    XCTAssertEqual(fake.binds, [], "it rebound a unit the engine had moved")
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-configured" }.map { $0.value("microphoneName") },
      [builtIn.name, usb.name, builtIn.name])
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-recovered" }.map { $0.value("microphoneId") },
      [usb.uid, builtIn.uid])
  }

  /// AVAudioEngine reports the moves it makes itself, such as binding the unit a graph just
  /// opened. On device QA each one rebuilt a working graph: every plain start took a second
  /// graph, and built-in → USB took three.
  func testASettlingGraphOutlastsAChangeThatKeepsItsInputAndRateButNotOneThatMovesIt()
    async throws
  {
    let fake = FakeMicrophone()
    let builtIn = fake.defaultInput
    let (microphone, output, feed) = try capture(fake)
    microphone.settleTime = .milliseconds(300)
    defer { feed.cancel() }
    // Reported as the graph starts, which stays on its input at its rate.
    fake.onStart = { fake.changed?(.configuration) }
    try await microphone.start(deviceUID: builtIn.uid, output: output)
    XCTAssertEqual(fake.opened.count, 1)

    // Reported as the engine moves the new graph's unit to another rate.
    fake.defaultInput = usb
    fake.onStart = {
      fake.rate = 16_000
      fake.changed?(.configuration)
    }
    fake.changed?(.input)
    try await until { self.recoveries() == 1 }

    // Reported mid-window as the graph stops delivering: buffers before it don't count.
    fake.defaultInput = builtIn
    fake.onStart = {
      Task { @MainActor in
        try? await Task.sleep(for: .milliseconds(100))
        fake.dead.insert(fake.taps.count - 1)
        fake.changed?(.configuration)
      }
    }
    fake.changed?(.input)
    try await until { self.recoveries() == 2 }
    microphone.stop()
    _ = try output.finish(reason: "user_stopped")
    let lines = try journal()
    XCTAssertEqual(
      lines.map(\.event),
      [
        "prepared", "microphone-configured", "microphone-interrupted", "microphone-configured",
        "microphone-startup-retry", "microphone-configured", "microphone-recovered",
        "microphone-interrupted", "microphone-configured", "microphone-startup-retry",
        "microphone-configured", "microphone-recovered", "ended",
      ])
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-configured" }.map { $0.value("inputSampleRate") },
      ["48000.0", "48000.0", "16000.0", "16000.0", "16000.0"])
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-startup-retry" }.map { $0.value("message") },
      [nil, "The microphone delivered no audio"])
  }

  /// On device QA an output change moved a 16 kHz USB microphone's unit onto a 48 kHz aggregate
  /// with the new output. The tap stopped with no configuration change, and only the stall
  /// watchdog rebuilt it, 2.3 s later.
  func testAPlainGraphTheEngineMovesToAnotherRateIsRebuiltWithoutWaitingForAStall() async throws {
    let fake = FakeMicrophone()
    fake.defaultInput = usb
    fake.rate = 16_000
    let (microphone, output, feed) = try capture(fake)
    microphone.stallSeconds = 30
    defer { feed.cancel() }
    try await microphone.start(deviceUID: usb.uid, output: output)
    fake.changed?(.output)
    fake.bound = InputBinding(device: 10, inputs: [usb.id], sampleRate: 48_000)
    try await until { self.recoveries() == 1 }

    // Onto the next output's aggregate at the same rate: the graph keeps recording.
    fake.changed?(.output)
    fake.bound = InputBinding(device: 11, inputs: [usb.id], sampleRate: 48_000)
    try await Task.sleep(for: .milliseconds(600))
    microphone.stop()
    _ = try output.finish(reason: "user_stopped")
    let lines = try journal()
    XCTAssertEqual(
      lines.map(\.event),
      [
        "prepared", "microphone-configured", "microphone-interrupted", "microphone-configured",
        "microphone-recovered", "ended",
      ])
    XCTAssertEqual(
      lines.first { $0.event == "microphone-interrupted" }?.value("reason"), "output-changed")
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-configured" }.map { $0.value("inputSampleRate") },
      ["16000.0", "48000.0"])
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-configured" }.map { $0.value("microphoneName") },
      [usb.name, usb.name])
    XCTAssertEqual(fake.binds, [])
  }

  /// AVAudioEngine moves a plain graph's unit to a new default input by itself, tens of
  /// milliseconds after the change. On device QA, binding it first lost that race: the journal
  /// named the engine's aggregate, or the tap's format went stale and the rebuild failed.
  func testARebuildWaitsForTheEngineToMoveItsUnitButNotForever() async throws {
    let fake = FakeMicrophone()
    let builtIn = fake.defaultInput
    fake.bound = InputBinding(device: builtIn.id, inputs: [builtIn.id], sampleRate: 48_000)
    let (microphone, output, feed) = try capture(fake)
    microphone.followSeconds = 0.3
    defer { feed.cancel() }
    try await microphone.start(deviceUID: builtIn.uid, output: output)
    let aggregate = InputBinding(device: 10, inputs: [usb.id], sampleRate: 48_000)
    fake.defaultInput = usb
    fake.changed?(.input)
    try await Task.sleep(for: .milliseconds(100))
    fake.bound = aggregate
    try await until { self.recoveries() == 1 }

    // The engine never moves it back to the built-in microphone: the rebuild binds it itself.
    fake.defaultInput = builtIn
    fake.changed?(.input)
    try await until { self.recoveries() == 2 }
    microphone.stop()
    _ = try output.finish(reason: "user_stopped")
    XCTAssertEqual(fake.openedOn.map(\.device), [builtIn.id, aggregate.device, aggregate.device])
    XCTAssertEqual(fake.binds, [builtIn.id])
    let lines = try journal()
    XCTAssertFalse(lines.contains { $0.event == "microphone-startup-retry" })
    XCTAssertEqual(
      lines.filter { $0.event == "microphone-configured" }.map { $0.value("microphoneName") },
      [builtIn.name, usb.name, builtIn.name])
  }

  /// On device QA rebinding a unit already on AVAudioEngine's aggregate of the default input
  /// made the engine report a change, and the journal sometimes named the aggregate itself.
  func testAUnitOnTheDefaultInputStaysWhereTheEngineBoundItAndIsNamedByThatInput() throws {
    let builtIn = InputDevice(
      id: 81, uid: "BuiltInMicrophoneDevice", name: "MacBook Pro Microphone")
    var binding = InputBinding(device: 184, inputs: [81], sampleRate: 48_000)
    var moveTo: InputBinding?
    var binds: [AudioObjectID] = []
    func follow() throws -> InputDevice {
      try followInput(builtIn, binding: { binding }) {
        binds.append($0)
        binding = moveTo ?? InputBinding(device: $0, inputs: [$0], sampleRate: 48_000)
      }
    }
    // The engine's aggregate of the built-in microphone and speakers.
    XCTAssertEqual(try follow(), builtIn)
    XCTAssertEqual(binds, [])

    // Left on a USB microphone's aggregate: bound to the built-in one.
    binding = InputBinding(device: 193, inputs: [113], sampleRate: 48_000)
    XCTAssertEqual(try follow(), builtIn)
    // Bound while the engine moved it onto a new aggregate of the built-in microphone.
    binding = InputBinding(device: 193, inputs: [113], sampleRate: 48_000)
    moveTo = InputBinding(device: 199, inputs: [81], sampleRate: 48_000)
    XCTAssertEqual(try follow(), builtIn)
    XCTAssertEqual(binds, [81, 81])

    // An aggregate of two inputs doesn't say which one a mono tap records, even after binding.
    binding = InputBinding(device: 206, inputs: [81, 74], sampleRate: 48_000)
    moveTo = binding
    XCTAssertThrowsError(try follow())
    XCTAssertEqual(binds, [81, 81, 81])
  }

  func testAMicrophoneThatStopsDeliveringIsRebuiltButASilentOneIsNot() async throws {
    let fake = FakeMicrophone()
    let (microphone, output, feed) = try capture(fake)
    defer { feed.cancel() }
    try await microphone.start(deviceUID: "BuiltInMicrophoneDevice", output: output)
    // Buffers of silence keep arriving: a quiet room, not a stall.
    try await Task.sleep(for: .milliseconds(1500))
    XCTAssertEqual(fake.opened.count, 1)
    XCTAssertFalse(try journal().contains { $0.event == "microphone-interrupted" })

    // The tap stops with no notification, and the first rebuilt graph delivers nothing either.
    // A late callback from the replaced tap doesn't make it count as delivering.
    fake.dead = [0, 1]
    fake.lateCallbacks = true
    try await until { self.recoveries() == 1 }
    XCTAssertEqual(fake.opened.count, 3)

    microphone.stop()
    fake.dead = [2]
    fake.changed?(.input)
    fake.changed?(.configuration)
    try await Task.sleep(for: .milliseconds(1000))
    XCTAssertEqual(fake.opened.count, 3, "it rebuilt after Stop")
    _ = try output.finish(reason: "user_stopped")
    let lines = try journal()
    XCTAssertEqual(
      lines.map(\.event),
      [
        "prepared", "microphone-configured", "microphone-interrupted", "microphone-configured",
        "microphone-startup-retry", "microphone-configured", "microphone-recovered", "ended",
      ])
    XCTAssertEqual(
      lines.first { $0.event == "microphone-interrupted" }?.value("reason"), "stalled")
    XCTAssertEqual(
      lines.first { $0.event == "microphone-startup-retry" }?.value("message"),
      "The microphone delivered no audio")
  }
}

private let usb = InputDevice(
  id: 2, uid: "AppleUSBAudioEngine:Unknown Manufacturer:Logitech Webcam C925e:5F3273EF:3",
  name: "Logitech Webcam C925e")

private func silence() -> AVAudioPCMBuffer {
  let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
  let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 480)!
  buffer.frameLength = 480
  buffer.floatChannelData![0].initialize(repeating: 0, count: 480)
  return buffer
}
