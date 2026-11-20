import AVFoundation
import AudioEngineGuard
import CoreAudio

/// Why the microphone is rebuilding, as `microphone-interrupted` reports it.
enum MicrophoneChange: String {
  /// AVAudioEngine reported a new device or format.
  case configuration = "configuration-changed"
  case input = "input-changed"
  case output = "output-changed"
  case restarted = "audio-service-restarted"
  /// The settled graph stopped delivering buffers.
  case stalled
}

/// A graph the backend built: the format its tap records in, and the input it opened. The input
/// is a physical device, never the private aggregate AVAudioEngine may record it through.
struct MicrophoneGraph {
  var format: AVAudioFormat
  var device: InputDevice?
}

/// Where a plain graph's input unit is. AVAudioEngine binds it to the default input itself, and
/// when the default output is another device, through a private aggregate of the two that it
/// remakes whenever either one changes.
struct InputBinding: Equatable {
  var device: AudioObjectID
  /// The devices whose input `device` brings in: itself, or an aggregate's sub-devices with input.
  var inputs: [AudioObjectID]
  /// The rate the unit's input runs at. A tap records only at this rate.
  var sampleRate: Double

  /// Whether the unit records `input` and nothing else. An aggregate of two inputs doesn't say
  /// which one a mono tap records, so it never counts.
  func carries(_ input: AudioObjectID) -> Bool { device == input || inputs == [input] }

  /// Whether the unit still records `graph`'s input at the rate its tap records.
  func records(_ graph: MicrophoneGraph) -> Bool {
    guard let input = graph.device else { return false }
    return carries(input.id) && sampleRate == graph.format.sampleRate
  }
}

/// Leaves a plain graph's input unit where AVAudioEngine bound it when that records `target`, and
/// otherwise binds it there. Binding a unit already on the default input's aggregate makes the
/// engine report a configuration change, and binding it while the engine moves it loses the race,
/// leaving the unit on the aggregate. Returns the physical input; throws rather than name an input
/// the unit can't be shown to record.
func followInput(
  _ target: InputDevice, binding: () throws -> InputBinding,
  bind: (AudioObjectID) throws -> Void
) throws -> InputDevice {
  if try binding().carries(target.id) { return target }
  try bind(target.id)
  guard try binding().carries(target.id) else {
    throw CaptureError.invalid("The microphone did not open the Mac's default input")
  }
  return target
}

/// The audio engine and Core Audio calls behind the microphone, replaceable in tests.
struct MicrophoneBackend {
  var defaultInput: @MainActor () -> InputDevice?
  var outputRoute: @MainActor () -> OutputRoute?
  /// Where a plain graph's input unit is now; nil while it can't be read.
  var binding: @MainActor () -> InputBinding?
  /// Builds a stopped graph on the Mac's default input, replacing the last one. Its tap delivers
  /// buffers with their host time in seconds.
  var open:
    @MainActor (
      _ voiceProcessing: Bool, _ deliver: @escaping @Sendable (AVAudioPCMBuffer, Double) -> Void
    ) throws -> MicrophoneGraph
  var start: @MainActor () throws -> Void
  var isRunning: @MainActor () -> Bool
  /// Never throws: a device mid-change can refuse any teardown call.
  var close: @MainActor () -> Void
  var voiceProcessingSettings:
    @MainActor () throws -> (duckingLevel: Int, advancedDucking: Bool, agc: Bool)
  /// Calls back on the main actor on a change that may need a rebuild. Returns the call that
  /// stops watching.
  var watch: @MainActor (@escaping @MainActor (MicrophoneChange) -> Void) -> @MainActor () -> Void
}

/// Owns the microphone. `CallAudio` owns the call track.
///
/// On speakers the microphone runs through Apple's voice processing, which cancels the call's
/// echo but also ducks every other app's audio, the call included, for as long as it runs. On
/// headphones there is no echo to cancel, so the microphone opens plain and nothing is ducked.
/// When the output route changes mid-recording, the microphone is rebuilt in the new route's mode.
/// It follows a new default input, and a graph that stops delivering buffers is rebuilt.
@MainActor
final class VoiceMicrophone {
  private let processing: MicrophoneProcessing
  private let backend: MicrophoneBackend
  /// The graph being settled or running; nil while none is built.
  private var graph: MicrophoneGraph?
  /// The mode the last graph opened in; nil until one is first built.
  private var voiceProcessing: Bool?
  private var unwatch: (@MainActor () -> Void)?
  private var watchdog: Task<Void, Never>?
  private let heard = TapClock()
  /// Startup or a recovery is rebuilding the graph; a change only restarts its wait.
  private var settling = false
  private var stopped = false
  private var configurationRevision = 0
  /// When the settling graph's quiet window began, in host seconds.
  private var quietSince = 0.0
  /// The settling graph already outlasted one configuration change.
  private var outlastedChange = false
  /// The output changed while a plain graph ran, so the engine may move its unit.
  private var outputChanged = false
  /// The quiet window a new graph must run through before it counts as settled.
  var settleTime: Duration = .milliseconds(750)
  /// How long a settled graph may go without a buffer before it is rebuilt. A running input
  /// delivers buffers in a silent room too, so only their absence counts, never silent samples.
  var stallSeconds = 2.0
  /// How long a plain rebuild waits for AVAudioEngine to move its unit to a new default input.
  var followSeconds = 0.5

  init(processing: MicrophoneProcessing = .automatic, backend: MicrophoneBackend? = nil) {
    self.processing = processing
    self.backend = backend ?? .engine(advancedDucking: processing.advancedDucking)
  }

  /// Whether the running graph uses voice processing; nil while none is built.
  var usesVoiceProcessing: Bool? { graph == nil ? nil : voiceProcessing }

  func start(deviceUID: String, output: AudioOutput) async throws {
    guard backend.defaultInput()?.uid == deviceUID else {
      throw CaptureError.invalid(
        "Choose this microphone as the Mac's default input before recording.")
    }
    settling = true
    unwatch = backend.watch { [weak self] change in self?.changed(change, output: output) }
    watchdog = Task { [weak self] in
      while !Task.isCancelled {
        try? await Task.sleep(for: .milliseconds(250))
        guard let self else { return }
        self.checkProgress(output: output)
      }
    }
    do {
      try await settle(output: output)
      settling = false
    } catch is CancellationError {
      settling = false
      stop()
      throw CancellationError()
    } catch {
      // A device changing as the recording starts gets the same treatment as one a second
      // later: remote audio records, and the microphone keeps silence until it settles.
      output.microphoneUnavailable(error)
      closeGraph()
      Task { [weak self] in await self?.retry(output: output, reported: true) }
    }
  }

  private func checkProgress(output: AudioOutput) {
    guard !stopped, !settling, let graph else { return }
    // When the output changes, AVAudioEngine moves a plain graph's unit onto an aggregate with
    // the new output. On device QA that moved a 16 kHz USB microphone to 48 kHz, and the tap
    // stopped with no configuration change.
    if voiceProcessing == false, let binding = backend.binding(), !binding.records(graph) {
      rebuild(outputChanged ? .output : .configuration, output: output)
      return
    }
    guard let last = heard.last, captureHostTime() - last > stallSeconds else { return }
    rebuild(.stalled, output: output)
  }

  /// A device change mid-recording, such as AirPods connecting or a headset unplugged, follows
  /// the Mac's current default input instead of ending the recording. Until the new graph
  /// settles, the microphone track keeps silence; remote audio never stops.
  private func changed(_ change: MicrophoneChange, output: AudioOutput) {
    guard !stopped else { return }
    switch change {
    case .output:
      // Voice processing follows a new output by itself. A plain graph's unit moves with it,
      // which `checkProgress` verifies. Only a change between headphones and speakers rebuilds.
      if voiceProcessing == false { outputChanged = true }
      guard let mode = voiceProcessing,
        microphoneUsesVoiceProcessing(on: backend.outputRoute(), processing: processing) != mode
      else { return }
    case .input:
      // Voice processing follows a new default input, and AVAudioEngine reports it. A plain
      // graph may not: on device QA it went silent, or recorded the new input under the old
      // name, with no configuration change.
      guard voiceProcessing != true, backend.defaultInput()?.id != graph?.device?.id else {
        return
      }
    case .configuration:
      // AVAudioEngine also reports the moves it makes itself, such as onto a new default input
      // just as its graph opened there. A plain graph that is still running on its input at its
      // rate restarts its quiet window instead of being rebuilt, once; it still needs buffers.
      if settling, voiceProcessing == false, !outlastedChange, let graph, backend.isRunning(),
        backend.binding()?.records(graph) == true
      {
        outlastedChange = true
        quietSince = captureHostTime()
        return
      }
    case .restarted, .stalled:
      break
    }
    rebuild(change, output: output)
  }

  private func rebuild(_ reason: MicrophoneChange, output: AudioOutput) {
    configurationRevision += 1
    guard !settling else { return }
    settling = true
    output.microphoneInterrupted(reason: reason.rawValue)
    Task { await retry(output: output, reported: false) }
  }

  /// AVAudioEngine moves a plain graph's unit to a new default input by itself, a moment after
  /// the change. Binding the unit first races that move, so wait for it, but not for long: if the
  /// engine never moves it, `followInput` binds it.
  private func awaitEngineFollowingDefaultInput() async throws {
    let deadline = captureHostTime() + followSeconds
    while captureHostTime() < deadline, let input = backend.defaultInput(),
      let binding = backend.binding(), binding.device != input.id,
      !binding.inputs.contains(input.id)
    {
      try await Task.sleep(for: .milliseconds(20))
      guard !stopped else { throw CancellationError() }
    }
  }

  /// Opening a Bluetooth microphone can renegotiate the output route. Rebuild from the current
  /// formats, then require a quiet window with buffers arriving before calling the microphone
  /// ready. A device that changes mid-rebuild makes AVAudioEngine fail the call, which counts as
  /// one more attempt.
  private func settle(output: AudioOutput) async throws {
    var failure: Error?
    for attempt in 1...4 {
      try Task.checkCancellation()
      guard !stopped else { throw CancellationError() }
      do {
        closeGraph()
        if voiceProcessing == false,
          !microphoneUsesVoiceProcessing(on: backend.outputRoute(), processing: processing)
        {
          try await awaitEngineFollowingDefaultInput()
        }
        let route = backend.outputRoute()
        let mode = microphoneUsesVoiceProcessing(on: route, processing: processing)
        let tap = heard.restart()
        let graph = try backend.open(mode) { [heard, output] pcm, hostTime in
          guard pcm.frameLength > 0 else { return }
          heard.mark(tap)
          output.appendMicrophone(pcm, hostTime: hostTime)
        }
        self.graph = graph
        voiceProcessing = mode
        outputChanged = false
        outlastedChange = false
        try output.microphoneConfigured(
          graph.format, device: graph.device, voiceProcessing: mode, processing: processing,
          route: route)
        let revision = configurationRevision
        try backend.start()
        quietSince = captureHostTime()
        var left = settleTime
        repeat {
          try await Task.sleep(for: left)
          guard !stopped else { throw CancellationError() }
          // A configuration change the graph outlasted restarts the window.
          left = settleTime - .seconds(captureHostTime() - quietSince)
        } while left > .zero
        if backend.isRunning(), configurationRevision == revision {
          if let last = heard.last, last > quietSince {
            if mode { reportVoiceProcessing(output) }
            return
          }
          failure = CaptureError.invalid("The microphone delivered no audio")
        } else {
          failure = nil
        }
      } catch is CancellationError {
        throw CancellationError()
      } catch {
        failure = error
      }
      try output.microphoneStartupRetry(attempt: attempt, error: failure)
      // Give a headset switching profiles a moment before the next attempt.
      if failure != nil { try await Task.sleep(for: .milliseconds(500)) }
    }
    throw failure
      ?? CaptureError.invalid("The audio device did not settle. Reconnect it and try again.")
  }

  /// Rebuilds the microphone until it settles or the recording stops. Ends the caller's settling.
  private func retry(output: AudioOutput, reported: Bool) async {
    defer { settling = false }
    var reported = reported
    while !stopped {
      do {
        try await settle(output: output)
        output.microphoneRecovered(
          deviceUID: graph?.device?.uid, voiceProcessing: voiceProcessing ?? true)
        return
      } catch {
        guard !stopped else { return }
        // No input is usable yet. Keep recording remote audio and try again shortly.
        if !reported { output.microphoneUnavailable(error) }
        reported = true
        closeGraph()
        try? await Task.sleep(for: .seconds(2))
      }
    }
  }

  /// The running unit's own settings, so a trial shows the ducking that applied, not only the
  /// one requested. Minimum is the lowest level the API offers; macOS has no way to turn it off.
  private func reportVoiceProcessing(_ output: AudioOutput) {
    do {
      let settings = try backend.voiceProcessingSettings()
      output.microphoneVoiceProcessing(
        duckingLevel: settings.duckingLevel, advancedDucking: settings.advancedDucking,
        agc: settings.agc)
    } catch {
      output.microphoneVoiceProcessingUnreadable(error)
    }
  }

  private func closeGraph() {
    backend.close()
    graph = nil
  }

  func stop() {
    stopped = true
    watchdog?.cancel()
    watchdog = nil
    unwatch?()
    unwatch = nil
    closeGraph()
  }
}

/// When the newest graph's tap last delivered, nil until it does. A late callback from a graph
/// already replaced doesn't count. Written on the tap's thread and read on the main actor.
private final class TapClock: @unchecked Sendable {
  private let lock = NSLock()
  private var tap = 0
  private var time: Double?

  var last: Double? { lock.withLock { time } }

  /// Starts waiting for a new graph's tap and returns its token.
  func restart() -> Int {
    lock.withLock {
      tap += 1
      time = nil
      return tap
    }
  }

  func mark(_ tap: Int) {
    let now = captureHostTime()
    lock.withLock {
      if tap == self.tap { time = now }
    }
  }
}

// MARK: - AVAudioEngine

extension MicrophoneBackend {
  /// The Mac's default input through one AVAudioEngine, kept for the whole recording.
  @MainActor
  static func engine(advancedDucking: Bool) -> MicrophoneBackend {
    let microphone = EngineMicrophone(advancedDucking: advancedDucking)
    return MicrophoneBackend(
      defaultInput: { InputDevice.current() },
      outputRoute: { OutputRoute.current() },
      binding: { try? microphone.binding() },
      open: { try microphone.open(voiceProcessing: $0, deliver: $1) },
      start: { try microphone.engine.guardedStart() },
      isRunning: { microphone.engine.isRunning },
      close: { microphone.clear() },
      voiceProcessingSettings: { try microphone.voiceProcessingSettings() },
      watch: { microphone.watch($0) })
  }
}

@MainActor
private final class EngineMicrophone {
  let engine = AVAudioEngine()
  private let advancedDucking: Bool
  /// Whether `engine` runs voice processing; nil until a graph is first built.
  private var voiceProcessing: Bool?
  private var tapped = false
  private var silence: AVAudioSourceNode?

  init(advancedDucking: Bool) {
    self.advancedDucking = advancedDucking
  }

  func open(
    voiceProcessing enabled: Bool,
    deliver: @escaping @Sendable (AVAudioPCMBuffer, Double) -> Void
  ) throws -> MicrophoneGraph {
    clear()
    try useVoiceProcessing(enabled)
    let input = try engine.guardedInputNode()
    let bound = enabled ? nil : try followDefaultInput(input)
    if enabled {
      // Pull the output side continuously so VoiceProcessingIO has a playback reference.
      // It renders only silence; neither captured channel is played back through this engine.
      let format = try microphoneFormat(input)
      let silence = AVAudioSourceNode(format: format) { isSilence, _, _, buffers in
        isSilence.pointee = true
        for buffer in UnsafeMutableAudioBufferListPointer(buffers) {
          if let data = buffer.mData { memset(data, 0, Int(buffer.mDataByteSize)) }
        }
        return noErr
      }
      try engine.guardedAttach(silence)
      self.silence = silence
      try engine.guardedConnect(silence, to: engine.outputNode, format: format)
    }
    // Read the format just before the tap. A Bluetooth headset switching between its playback
    // and headset profiles changes rate under us; if it still does, the tap fails and settles again.
    let format = try microphoneFormat(input)
    try input.guardedInstallTap(onBus: 0, bufferSize: 1024, format: format) { pcm, time in
      // A buffer without a host time can't be placed; drop it rather than end the recording.
      guard time.isHostTimeValid else { return }
      deliver(pcm, AVAudioTime.seconds(forHostTime: time.hostTime))
    }
    tapped = true
    // Voice processing opens the Mac's default input itself.
    return MicrophoneGraph(format: format, device: enabled ? InputDevice.current() : bound)
  }

  /// A plain graph's input unit can be left off the default input: on device QA it went silent
  /// after the default moved from a USB microphone to the built-in one.
  private func followDefaultInput(_ input: AVAudioInputNode) throws -> InputDevice {
    guard let target = InputDevice.current() else {
      throw CaptureError.invalid("No microphone is available")
    }
    guard let unit = input.audioUnit else { return target }
    return try followInput(target, binding: { try binding(input) }) { device in
      var id = device
      let status = AudioUnitSetProperty(
        unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &id,
        UInt32(MemoryLayout<AudioObjectID>.size))
      guard status == noErr else {
        throw CoreAudioError(call: "Opening the default input", status: status)
      }
    }
  }

  func binding() throws -> InputBinding {
    try binding(engine.guardedInputNode())
  }

  private func binding(_ input: AVAudioInputNode) throws -> InputBinding {
    guard let unit = input.audioUnit else {
      throw CaptureError.invalid("The microphone has no input unit")
    }
    let device = try boundDevice(unit)
    return InputBinding(
      device: device, inputs: InputDevice.inputs(of: device),
      sampleRate: try input.guardedInputFormat(forBus: 0).sampleRate)
  }

  private func boundDevice(_ unit: AudioUnit) throws -> AudioObjectID {
    var id = AudioObjectID(kAudioObjectUnknown)
    var size = UInt32(MemoryLayout<AudioObjectID>.size)
    let status = AudioUnitGetProperty(
      unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &id, &size)
    guard status == noErr else {
      throw CoreAudioError(call: "Reading the microphone's device", status: status)
    }
    return id
  }

  /// Switch modes on the stopped engine. Replacing the engine instead hung AVAudioEngine's
  /// input setup when switching back to voice processing.
  private func useVoiceProcessing(_ enabled: Bool) throws {
    guard enabled != voiceProcessing else { return }
    voiceProcessing = nil
    try engine.guardedInputNode().guardedSetVoiceProcessing(
      enabled, advancedDucking: advancedDucking)
    voiceProcessing = enabled
  }

  /// The mono format the tap records in. VPIO's aggregate hardware format can expose extra
  /// playback-reference channels, so voice processing taps its processed uplink, never a downmix
  /// of the aggregate. A plain microphone taps at its hardware rate: after leaving voice
  /// processing on a Bluetooth headset, the node's output side kept the headset's 44.1 kHz
  /// playback rate while the hardware ran its 16 kHz headset profile, and the tap refused it.
  private func microphoneFormat(_ input: AVAudioInputNode) throws -> AVAudioFormat {
    let side =
      voiceProcessing == true
      ? try input.guardedOutputFormat(forBus: 0) : try input.guardedInputFormat(forBus: 0)
    guard
      let format = AVAudioFormat(standardFormatWithSampleRate: side.sampleRate, channels: 1),
      format.sampleRate > 0
    else {
      throw CaptureError.invalid("The microphone has no usable audio format")
    }
    return format
  }

  func voiceProcessingSettings() throws -> (duckingLevel: Int, advancedDucking: Bool, agc: Bool) {
    var ducking = AVAudioVoiceProcessingOtherAudioDuckingConfiguration()
    var agc: ObjCBool = false
    try engine.guardedInputNode().guardedReadVoiceProcessing(ducking: &ducking, agc: &agc)
    return (ducking.duckingLevel.rawValue, ducking.enableAdvancedDucking.boolValue, agc.boolValue)
  }

  /// Never throws: a device mid-change can refuse any of these, and the next open starts over.
  func clear() {
    try? engine.guardedStop()
    if tapped {
      try? engine.guardedInputNode().guardedRemoveTap(onBus: 0)
      tapped = false
    }
    if let silence {
      try? engine.guardedDetach(silence)
      self.silence = nil
    }
  }

  /// AVAudioEngine's own configuration change, the output route, a new default input, and
  /// coreaudiod restarting, which destroys every audio unit.
  func watch(_ changed: @escaping @MainActor (MicrophoneChange) -> Void) -> @MainActor () -> Void {
    let observer = NotificationCenter.default.addObserver(
      forName: .AVAudioEngineConfigurationChange, object: engine, queue: nil
    ) { _ in
      // AVAudioEngine posts on its own queue. Reconfigure only on the owning actor.
      Task { @MainActor in changed(.configuration) }
    }
    let route = OutputRouteWatcher()
    route.start { changed(.output) }
    let system = AudioObjectID(kAudioObjectSystemObject)
    let watched: [(AudioObjectPropertySelector, MicrophoneChange)] = [
      (kAudioHardwarePropertyDefaultInputDevice, .input),
      (kAudioHardwarePropertyServiceRestarted, .restarted),
    ]
    let listeners = watched.map { selector, change in
      let listener: AudioObjectPropertyListenerBlock = { _, _ in
        Task { @MainActor in changed(change) }
      }
      var address = AudioObjectPropertyAddress(
        mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain)
      AudioObjectAddPropertyListenerBlock(system, &address, .main, listener)
      return (address, listener)
    }
    return {
      NotificationCenter.default.removeObserver(observer)
      route.stop()
      for (address, listener) in listeners {
        var address = address
        AudioObjectRemovePropertyListenerBlock(system, &address, .main, listener)
      }
    }
  }
}
