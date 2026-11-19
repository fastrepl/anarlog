import AVFoundation
import AppKit
import CoreMedia
import Foundation

public func captureHostTime() -> Double { CMClockGetTime(CMClockGetHostTimeClock()).seconds }

public func captureMicrophones() -> [AVCaptureDevice] {
  AVCaptureDevice.DiscoverySession(
    deviceTypes: [.microphone, .external], mediaType: .audio, position: .unspecified
  ).devices
}

/// The Mac's default input when `requested` names it (by UID or name), or no request was made.
/// The helper never changes the default input, so any other selection is refused.
public func defaultMicrophone(matching requested: String?) throws -> AVCaptureDevice {
  guard let device = AVCaptureDevice.default(for: .audio), device.isConnected else {
    throw CaptureError.refused(
      code: "microphone_unavailable", message: "No microphone is available")
  }
  if let requested, requested != device.uniqueID, requested != device.localizedName {
    throw CaptureError.refused(
      code: "microphone_not_default",
      message: "The selected microphone is not the Mac's default input")
  }
  return device
}

/// Forwards each run a track writes to Anarlog. Writer-queue owned.
private final class LivePCM {
  let live: LiveOutput?
  private(set) var overrun = false

  init(_ live: LiveOutput?) { self.live = live }

  func send(track: UInt8, _ pcm: AVAudioPCMBuffer, frame: Int64) {
    guard let live, !overrun, let samples = pcm.floatChannelData?[0] else { return }
    let run = UnsafeBufferPointer(start: samples, count: Int(pcm.frameLength))
    for message in WireFrame.encode(track: track, frame: frame, samples: run)
    where !live.send(message) {
      overrun = true
      return
    }
  }

  func refuse() { overrun = true }
}

// Track state belongs to writer; error access is protected by errorLock.
public final class AudioOutput: @unchecked Sendable {
  /// How far behind the host clock the live watermark runs. Device callbacks and the converter
  /// deliver within it; anything later still reaches the CAF track but not the live stream.
  public static let liveLatency = 0.5
  private let writer = DispatchQueue(label: "so.anarlog.meeting-capture.writer")
  private let slots = DispatchSemaphore(value: 32)
  private let errorLock = NSLock()
  private var failure: Error?
  private var timeline: CaptureTimeline
  private var closed = false
  private var lastMeeting: MeetingObservation?
  private var lastUnknownReason: String?
  private var droppingMicrophone = false
  private var droppingRemote = false
  private var overrunReported = false
  private var ticker: DispatchSourceTimer?
  private var watermark: Int64 = 0
  private let livePCM: LivePCM
  private let microphone: AudioTrack
  private let remote: AudioTrack
  private let journal: FileHandle
  public let origin: Double
  public let captureID: String

  public var error: Error? {
    errorLock.lock()
    defer { errorLock.unlock() }
    return failure
  }

  public init(
    directory: URL, captureID: String, processID: Int32, bundleID: String,
    microphoneID: String?, live: LiveOutput? = nil
  ) throws {
    origin = captureHostTime()
    self.captureID = captureID
    timeline = try CaptureTimeline(origin: origin)
    let livePCM = LivePCM(live)
    self.livePCM = livePCM
    microphone = try AudioTrack(url: directory.appendingPathComponent("microphone.caf")) {
      pcm, frame in livePCM.send(track: 0, pcm, frame: frame)
    }
    remote = try AudioTrack(url: directory.appendingPathComponent("remote.caf")) { pcm, frame in
      livePCM.send(track: 1, pcm, frame: frame)
    }
    let url = directory.appendingPathComponent("capture.journal")
    guard FileManager.default.createFile(atPath: url.path, contents: nil) else {
      throw CaptureError.invalid("Cannot create recording journal")
    }
    journal = try FileHandle(forWritingTo: url)
    try log(
      EventLine("prepared").with("version", 1).with("captureId", captureID)
        .with("originHostSeconds", origin)
        .with("createdAt", ISO8601DateFormatter().string(from: Date()))
        .with("processId", processID).with("bundleId", bundleID)
        .with("sampleRate", Int(AudioTrack.sampleRate))
        .with("microphoneId", microphoneID ?? "default")
        .with("scope", processID == 0 ? "system" : "application"))
  }

  /// Journaled, synced, and sent to Anarlog. Writer-queue (or init) only.
  private func log(_ line: EventLine) throws {
    var data = Data(line.text.utf8)
    data.append(10)
    try journal.write(contentsOf: data)
    try journal.synchronize()
    sendLive(line)
  }

  private func sendLive(_ line: EventLine) {
    if let live = livePCM.live, !livePCM.overrun,
      !live.send(line, wait: .now() + .seconds(line.event == "ended" ? 5 : 1))
    {
      livePCM.refuse()
    }
    reportOverrun()
  }

  private func reportOverrun() {
    guard livePCM.overrun, !overrunReported else { return }
    overrunReported = true
    fail(
      CaptureError.refused(
        code: "live_output_overrun", message: "Anarlog stopped reading live audio"))
    var data = Data(EventLine("live-output-overrun").with("atMs", elapsed()).text.utf8)
    data.append(10)
    try? journal.write(contentsOf: data)
    try? journal.synchronize()
  }

  public func frame(at time: Double) -> Int64 {
    Int64(((time - origin) * AudioTrack.sampleRate).rounded())
  }

  /// The capture is running. Starts the live watermark, which advances through pauses and gaps.
  public func started(_ line: EventLine) throws {
    try writer.sync {
      try log(line.with("atMs", elapsed()))
      let ticker = DispatchSource.makeTimerSource(queue: writer)
      ticker.schedule(deadline: .now(), repeating: .milliseconds(100), leeway: .milliseconds(20))
      ticker.setEventHandler { [weak self] in self?.tick() }
      ticker.resume()
      self.ticker = ticker
    }
  }

  private func tick() {
    guard !closed, let live = livePCM.live, !livePCM.overrun else { return }
    let frame = Int64(
      ((captureHostTime() - origin - Self.liveLatency) * AudioTrack.sampleRate).rounded(.down))
    guard frame > watermark else { return }
    watermark = frame
    if !live.send(EventLine("tick").with("frame", frame)) { livePCM.refuse() }
    reportOverrun()
  }

  func microphoneConfigured(
    _ format: AVAudioFormat, device: InputDevice?, voiceProcessing: Bool,
    processing: MicrophoneProcessing, route: OutputRoute?
  ) throws {
    try writer.sync {
      try log(
        EventLine("microphone-configured").with("microphoneId", device?.uid)
          .with("microphoneName", device?.name)
          .with("voiceProcessing", voiceProcessing)
          .with("processing", processing.rawValue)
          .with("advancedDucking", voiceProcessing && processing.advancedDucking)
          .with("inputSampleRate", format.sampleRate)
          .with("inputChannels", Int(format.channelCount))
          .with("output", route?.name ?? "unknown").with("headphones", route?.isHeadphones ?? false)
          .with("outputTransport", route?.transportCode ?? "unknown").with("atMs", elapsed()))
    }
  }

  func microphoneStartupRetry(attempt: Int, error: Error?) throws {
    let line = EventLine("microphone-startup-retry").with("attempt", attempt)
      .with("message", error?.localizedDescription).with("atMs", elapsed())
    try writer.sync { try log(line) }
  }

  func microphoneInterrupted(reason: String) {
    note(EventLine("microphone-interrupted").with("reason", reason))
  }

  func microphoneRecovered(deviceUID: String?, voiceProcessing: Bool) {
    note(
      EventLine("microphone-recovered").with("microphoneId", deviceUID ?? "default")
        .with("voiceProcessing", voiceProcessing))
  }

  func microphoneVoiceProcessing(duckingLevel: Int, advancedDucking: Bool, agc: Bool) {
    note(
      EventLine("microphone-voice-processing").with("duckingLevel", duckingLevel)
        .with("advancedDucking", advancedDucking).with("agc", agc))
  }

  func microphoneVoiceProcessingUnreadable(_ error: Error) {
    note(EventLine("microphone-voice-processing").with("message", error.localizedDescription))
  }

  func microphoneUnavailable(_ error: Error) {
    note(EventLine("microphone-unavailable").with("message", error.localizedDescription))
  }

  func callAudioConfigured(scope: String, processes: Int, route: OutputRoute?) {
    note(
      EventLine("call-audio-configured").with("scope", scope).with("processes", processes)
        .with("output", route?.name ?? "unknown").with("headphones", route?.isHeadphones ?? false))
  }

  func callAudioProcessesChanged(count: Int) {
    note(EventLine("call-audio-processes-changed").with("processes", count))
  }

  func callAudioInterrupted(reason: String) {
    note(EventLine("call-audio-interrupted").with("reason", reason))
  }

  func callAudioUnavailable(_ error: Error) {
    note(EventLine("call-audio-unavailable").with("message", error.localizedDescription))
  }

  /// A device-change note. It never reopens a finished journal.
  private func note(_ line: EventLine) {
    writer.sync {
      guard !closed else { return }
      do { try log(line.with("atMs", elapsed())) } catch { fail(error) }
    }
  }

  public func pause() throws {
    let time = captureHostTime()
    try writer.sync {
      guard !closed else { throw CaptureError.invalid("Capture is closed") }
      try timeline.pause(at: time)
      lastMeeting = nil
      lastUnknownReason = nil
      try log(
        EventLine("paused").with("frame", frame(at: time)).with("atMs", (time - origin) * 1000))
    }
  }

  public func resume() throws {
    let time = captureHostTime()
    try writer.sync {
      guard !closed else { throw CaptureError.invalid("Capture is closed") }
      try timeline.resume(at: time)
      try log(
        EventLine("resumed").with("frame", frame(at: time)).with("atMs", (time - origin) * 1000))
    }
  }

  /// Store only projected meeting metadata. Missing UI is unknown, not a meeting-ended signal.
  /// The observation is stamped with the time its read began.
  public func observeMeeting(
    _ meeting: MeetingObservation?, unknownReason: String?, startedAt: Double
  ) {
    writer.sync {
      guard !closed, timeline.acceptsObservation(startedAt: startedAt) else { return }
      let line =
        (meeting.map(EventLine.meetingObservation)
        ?? EventLine("meeting-observation-unavailable").with("reason", unknownReason ?? "unknown"))
        .with("frame", frame(at: startedAt)).with("atMs", (startedAt - origin) * 1000)
      do {
        if meeting != lastMeeting || unknownReason != lastUnknownReason {
          try log(line)
        } else {
          // A stable roster still refreshes the speaker-evidence lease on every successful read.
          sendLive(line)
        }
      } catch { fail(error) }
      lastMeeting = meeting
      lastUnknownReason = unknownReason
    }
  }

  private func elapsed() -> Double { (captureHostTime() - origin) * 1000 }

  /// Stop admitting samples at the command's timestamp, before the devices are torn down.
  public func end() throws {
    let time = captureHostTime()
    try writer.sync {
      try timeline.stop(at: time)
      try log(
        EventLine("stopping").with("frame", frame(at: time)).with("atMs", (time - origin) * 1000))
    }
  }

  public func finish(reason: String) throws -> EventLine {
    let result: EventLine = try writer.sync {
      guard !closed else { throw CaptureError.invalid("Capture already finalized") }
      closed = true
      ticker?.cancel()
      ticker = nil
      defer {
        try? microphone.close()
        try? remote.close()
        try? journal.close()
      }
      try microphone.flush()
      try remote.flush()
      // Preserve trailing pauses and silence through the Stop boundary on both tracks.
      let stoppedFrames = Int64(((timeline.stoppedAt ?? origin) - origin) * AudioTrack.sampleRate)
      let length = max(stoppedFrames, microphone.writtenFrames, remote.writtenFrames)
      try microphone.pad(to: length)
      try remote.pad(to: length)
      try microphone.close()
      try remote.close()
      let result = EventLine("ended").with("reason", reason).with("frame", length)
        .with("atMs", elapsed()).with("frames", length)
        .with("sampleRate", Int(AudioTrack.sampleRate))
        .with("microphoneFrames", microphone.capturedFrames)
        .with("remoteFrames", remote.capturedFrames)
        .with("microphonePeak", microphone.peak).with("remotePeak", remote.peak)
      try log(result)
      return result
    }
    livePCM.live?.flush()
    return result
  }

  func fail(_ error: Error) {
    errorLock.lock()
    if failure == nil { failure = error }
    errorLock.unlock()
  }

  /// The engine reuses its tap buffers. Copy before dispatching to the serial writer.
  func appendMicrophone(_ buffer: AVAudioPCMBuffer, hostTime: Double) {
    guard buffer.frameLength > 0 else { return }
    do {
      append(
        try AudioTrack.copy(buffer, range: 0..<Int(buffer.frameLength)), hostTime: hostTime,
        remote: false)
    } catch { fail(error) }
  }

  /// `buffer` is the caller's own copy of the call tap's samples.
  func appendRemote(_ buffer: AVAudioPCMBuffer, hostTime: Double) {
    guard buffer.frameLength > 0 else { return }
    append(buffer, hostTime: hostTime, remote: true)
  }

  private func append(_ buffer: AVAudioPCMBuffer, hostTime: Double, remote isRemote: Bool) {
    guard slots.wait(timeout: .now()) == .success else {
      fail(
        CaptureError.refused(
          code: "writer_overrun", message: "Audio writer could not keep up; recording stopped"))
      return
    }
    writer.async { [self] in
      defer { slots.signal() }
      guard !closed, error == nil else { return }
      let track = isRemote ? remote : microphone
      do {
        guard hostTime.isFinite, abs(hostTime - captureHostTime()) < 10 else {
          throw CaptureError.invalid("Audio timestamp is not on the host clock")
        }
        try track.append(buffer, hostTime: hostTime, timeline: timeline)
        if isRemote { droppingRemote = false } else { droppingMicrophone = false }
        reportOverrun()
      } catch let error as AudioWriteError {
        // The disk, not the device: every later buffer would be lost too.
        fail(error)
      } catch {
        // A buffer from a device mid-change can be unusable. Its time stays silence on the
        // track; the recording goes on. Note only the first of a run.
        track.reset()
        if !(isRemote ? droppingRemote : droppingMicrophone) {
          try? log(
            EventLine(isRemote ? "call-audio-buffer-dropped" : "microphone-buffer-dropped")
              .with("message", error.localizedDescription).with("atMs", elapsed()))
        }
        if isRemote { droppingRemote = true } else { droppingMicrophone = true }
      }
    }
  }
}

extension EventLine {
  /// A roster read as Anarlog's adapter reads it. Participants keep their order; `local` is
  /// `unknown` when the UI does not say.
  public static func meetingObservation(_ meeting: MeetingObservation) -> EventLine {
    var line = EventLine("meeting-observation").with("platform", meeting.platform)
      .with("scopeId", meeting.scopeID).with("rosterVisible", meeting.rosterVisible)
      .with("participantCount", meeting.participantCount.map(String.init))
      .with("speakerStatus", meeting.speakerStatus).with("source", meeting.source)
    for participant in meeting.participants {
      line = line.with("participant.id", participant.id)
        .with("participant.name", participant.displayName)
        .with("participant.local", participant.isLocal.map { $0 ? "true" : "false" } ?? "unknown")
    }
    for speaker in meeting.speakers {
      line = line.with("speaker.id", speaker.participantID).with(
        "speaker.name", speaker.displayName)
    }
    return line
  }
}

@MainActor
public final class NativeCapture {
  private let callAudio: CallAudio
  private let voiceMicrophone: VoiceMicrophone
  private let processing: MicrophoneProcessing
  public let output: AudioOutput
  public let microphone: AVCaptureDevice
  private var stopped = false

  private init(
    callAudio: CallAudio, output: AudioOutput, voiceMicrophone: VoiceMicrophone,
    processing: MicrophoneProcessing, microphone: AVCaptureDevice
  ) {
    self.voiceMicrophone = voiceMicrophone
    self.callAudio = callAudio
    self.output = output
    self.processing = processing
    self.microphone = microphone
  }

  /// Whether the microphone runs through voice processing now, or would on the current route
  /// while it is still settling.
  public var microphoneVoiceProcessing: Bool {
    voiceMicrophone.usesVoiceProcessing
      ?? microphoneUsesVoiceProcessing(on: OutputRoute.current(), processing: processing)
  }

  /// `directory` is Anarlog's: it must exist, be private, and be empty.
  public static func checkDirectory(_ directory: URL) throws {
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: directory.path, isDirectory: &isDirectory),
      isDirectory.boolValue
    else {
      throw CaptureError.refused(
        code: "staging_invalid", message: "The capture directory is missing")
    }
    let attributes = try FileManager.default.attributesOfItem(atPath: directory.path)
    let permissions = (attributes[.posixPermissions] as? NSNumber)?.intValue ?? 0o777
    guard permissions & 0o077 == 0,
      (attributes[.ownerAccountID] as? NSNumber)?.uint32Value == getuid()
    else {
      throw CaptureError.refused(
        code: "staging_invalid", message: "The capture directory is not private")
    }
    guard try FileManager.default.contentsOfDirectory(atPath: directory.path).isEmpty else {
      throw CaptureError.refused(
        code: "staging_invalid", message: "The capture directory already holds a recording")
    }
  }

  public static func start(
    scope: CallAudioScope, directory: URL, captureID: String, microphoneID: String? = nil,
    microphoneProcessing: MicrophoneProcessing = .automatic, live: LiveOutput? = nil
  ) async throws -> NativeCapture {
    try checkDirectory(directory)
    // The helper never prompts: Anarlog asks for microphone access in its own permission flow.
    guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
      throw CaptureError.refused(
        code: "microphone_permission", message: "Microphone permission is required")
    }
    let microphone = try defaultMicrophone(matching: microphoneID)
    // Never broaden scope if the requested app is absent.
    if case .application(let pid, _) = scope {
      guard let app = NSRunningApplication(processIdentifier: pid), !app.isTerminated else {
        throw CaptureError.refused(
          code: "call_app_unavailable", message: "The selected application is unavailable")
      }
    }
    let processID: Int32
    let bundleID: String
    switch scope {
    case .application(let pid, let id): (processID, bundleID) = (pid, id)
    case .system: (processID, bundleID) = (0, "")
    }
    let output = try AudioOutput(
      directory: directory, captureID: captureID, processID: processID, bundleID: bundleID,
      microphoneID: microphone.uniqueID, live: live)
    let voiceMicrophone = VoiceMicrophone(processing: microphoneProcessing)
    let callAudio = CallAudio(scope: scope, output: output)
    do {
      try await voiceMicrophone.start(deviceUID: microphone.uniqueID, output: output)
      // The first tap asks macOS for System Audio Recording, never Screen Recording.
      callAudio.start()
      return NativeCapture(
        callAudio: callAudio, output: output, voiceMicrophone: voiceMicrophone,
        processing: microphoneProcessing, microphone: microphone)
    } catch {
      voiceMicrophone.stop()
      callAudio.stop()
      try? output.end()
      _ = try? output.finish(reason: "start_failed")
      throw error
    }
  }

  /// Tearing down a tap or the microphone never fails the stop, so both tracks always finish.
  public func stop(reason: String) async throws -> EventLine {
    guard !stopped else { throw CaptureError.invalid("Capture already stopped") }
    stopped = true
    try output.end()
    voiceMicrophone.stop()
    callAudio.stop()
    return try output.finish(reason: reason)
  }
}
