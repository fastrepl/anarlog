import AVFoundation
import CoreAudio
import Foundation

/// Whose sound the call track records.
public enum CallAudioScope: Equatable, Sendable {
  /// One app, its helper processes included: a browser plays a call from its audio service.
  case application(pid: Int32, bundleID: String)
  /// Every app on this Mac except the recorder itself.
  case system
}

/// A Core Audio process object: a process that has opened audio, with its PID and bundle ID.
struct AudioProcess: Equatable {
  let object: AudioObjectID
  let pid: Int32
  let bundleID: String
  /// Whether it is playing sound now.
  var isPlaying = false
}

/// What one process tap mixes: these processes, or everything but them.
struct TapPlan: Equatable {
  var excluding: Bool
  var processes: [AudioObjectID]
}

/// The processes a recording of `scope` taps or leaves out.
///
/// An app's sound is its own process tree: the app and every process it started. Chrome plays a
/// Meet call from `com.google.Chrome.helper`, and Dia from `company.thebrowser.browser.helper`, so
/// a helper's bundle ID needn't match the app's. A separate browser, or a second Chrome with its
/// own profile, was started by launchd and stays out.
func tapPlan(
  _ scope: CallAudioScope, processes: [AudioProcess], recorder: Int32 = getpid(),
  parent: (Int32) -> Int32? = parentProcessID
) -> TapPlan {
  switch scope {
  case .system:
    return TapPlan(
      excluding: true, processes: processes.filter { $0.pid == recorder }.map(\.object))
  case .application(let pid, _):
    return TapPlan(
      excluding: false,
      processes: processes.filter { $0.pid == pid || descends($0.pid, from: pid, parent: parent) }
        .map(\.object))
  }
}

/// Whether a tap of `plan` should be receiving sound: one of its processes is playing. A tap
/// receives no buffers at all while its processes are silent.
func expectsAudio(_ plan: TapPlan, processes: [AudioProcess]) -> Bool {
  processes.contains { process in
    process.isPlaying && plan.processes.contains(process.object) != plan.excluding
  }
}

/// A mono, private tap that leaves the tapped sound playing as before.
func tapDescription(_ plan: TapPlan, uuid: UUID) -> CATapDescription {
  let description =
    plan.excluding
    ? CATapDescription(monoGlobalTapButExcludeProcesses: plan.processes)
    : CATapDescription(monoMixdownOfProcesses: plan.processes)
  description.uuid = uuid
  description.name = "Anarlog meeting call audio"
  description.isPrivate = true
  description.muteBehavior = CATapMuteBehavior.unmuted
  return description
}

/// Whether `pid` was started by `ancestor`, directly or through its children.
func descends(_ pid: Int32, from ancestor: Int32, parent: (Int32) -> Int32?) -> Bool {
  var current = parent(pid)
  for _ in 0..<32 {
    guard let next = current, next > 1 else { return false }
    if next == ancestor { return true }
    current = parent(next)
  }
  return false
}

/// A running process's parent, or nil once it has exited.
func parentProcessID(_ pid: Int32) -> Int32? {
  var info = kinfo_proc()
  var size = MemoryLayout<kinfo_proc>.stride
  var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, pid]
  guard sysctl(&mib, 4, &info, &size, nil, 0) == 0, size > 0 else { return nil }
  return info.kp_eproc.e_ppid
}

/// One running tap. `close` never throws: a device mid-change can refuse any teardown call.
struct CallAudioSession {
  var update: (TapPlan) -> Bool
  var close: () -> Void
}

/// The Core Audio calls behind the call track, replaceable in tests.
struct CallAudioBackend {
  var processes: () -> [AudioProcess]
  /// Opens a tap and its aggregate device. Buffers arrive with their host time in seconds;
  /// `formatChanged` runs on the main actor when the tap's format changes under it.
  var open:
    (
      TapPlan, _ deliver: @escaping (AVAudioPCMBuffer, Double) -> Void,
      _ formatChanged: @escaping @MainActor () -> Void
    ) throws -> CallAudioSession
  /// Calls back on the main actor when the default output or the process list changes.
  /// Returns the call that stops watching.
  var watch: (@escaping @MainActor (CallAudioChange) -> Void) -> () -> Void
}

enum CallAudioChange: String {
  case output = "output-changed"
  case processes = "processes-changed"
  case format = "format-changed"
  case restarted = "audio-service-restarted"
  case stalled = "stalled"
}

/// Records the call track through a Core Audio process tap. It never ends the recording: when
/// the output device, its format or the tapped processes change, it rebuilds the tap, and the
/// track keeps silence for the moments it had none.
@MainActor
final class CallAudio {
  private let scope: CallAudioScope
  private let output: AudioOutput
  private let backend: CallAudioBackend
  private var session: CallAudioSession?
  private var plan: TapPlan?
  private var unwatch: (() -> Void)?
  private var stopped = false
  private var rebuilding = false
  private var pending = false
  private var watchdog: Task<Void, Never>?
  private let heard = HeardClock()
  /// The quiet time after which a tap that had sound, while its processes still play, counts as
  /// stalled and is rebuilt.
  var stallSeconds = 3.0
  /// The wait between attempts while no tap can be opened.
  var retryDelay: Duration = .seconds(1)

  init(scope: CallAudioScope, output: AudioOutput, backend: CallAudioBackend = .coreAudio) {
    self.scope = scope
    self.output = output
    self.backend = backend
  }

  /// Never throws or waits on the tap. One that can't open yet leaves the call track silent and
  /// retries in the background, so the recording starts and Stop can still end it.
  func start() {
    unwatch = backend.watch { [weak self] change in self?.changed(change) }
    if !openTap() {
      Task { await rebuild(reason: nil) }
    }
    watchdog = Task { @MainActor [weak self] in
      while let self, !self.stopped {
        try? await Task.sleep(for: .milliseconds(500))
        // Only a tap that delivered since it opened: one that never did is silent, not stalled,
        // and rebuilding it again would change nothing.
        if let plan = self.plan, !self.rebuilding, let last = self.heard.last,
          captureHostTime() - last > self.stallSeconds,
          expectsAudio(plan, processes: self.backend.processes())
        {
          self.changed(.stalled)
        }
      }
    }
  }

  private func changed(_ change: CallAudioChange) {
    guard !stopped else { return }
    if change == .processes {
      // A tap being rebuilt reads the process list afresh when it opens.
      guard let session, !rebuilding else { return }
      let next = tapPlan(scope, processes: backend.processes())
      guard next != plan else { return }
      if session.update(next) {
        plan = next
        output.callAudioProcessesChanged(count: next.processes.count)
        return
      }
    }
    Task { await rebuild(reason: change) }
  }

  /// Closes the current tap and opens a new one until it works or the recording stops.
  private func rebuild(reason: CallAudioChange?) async {
    guard !rebuilding else {
      pending = true
      return
    }
    rebuilding = true
    defer { rebuilding = false }
    if let reason { output.callAudioInterrupted(reason: reason.rawValue) }
    var reported = reason == nil
    repeat {
      pending = false
      closeTap()
      // Let a device that is switching settle before tapping it again.
      try? await Task.sleep(for: .milliseconds(250))
      while !stopped, !openTap(report: !reported) {
        reported = true
        try? await Task.sleep(for: retryDelay)
      }
    } while pending && !stopped
  }

  @discardableResult
  private func openTap(report: Bool = true) -> Bool {
    guard !stopped else { return true }
    let next = tapPlan(scope, processes: backend.processes())
    do {
      let heard = heard
      let output = output
      session = try backend.open(
        next,
        { buffer, hostTime in
          heard.mark()
          output.appendRemote(buffer, hostTime: hostTime)
        },
        { [weak self] in self?.changed(.format) })
      plan = next
      heard.reset()
      output.callAudioConfigured(
        scope: scope == .system ? "system" : "application", processes: next.processes.count,
        route: OutputRoute.current())
      return true
    } catch {
      if report { output.callAudioUnavailable(error) }
      return false
    }
  }

  private func closeTap() {
    session?.close()
    session = nil
    plan = nil
  }

  func stop() {
    stopped = true
    watchdog?.cancel()
    unwatch?()
    unwatch = nil
    closeTap()
  }
}

/// The host time of the tap's last buffer, nil until the open tap delivers one. Written on the
/// tap's queue and read on the main actor.
private final class HeardClock: @unchecked Sendable {
  private let lock = NSLock()
  private var time: Double?

  var last: Double? {
    lock.lock()
    defer { lock.unlock() }
    return time
  }

  func mark() {
    lock.lock()
    time = captureHostTime()
    lock.unlock()
  }

  func reset() {
    lock.lock()
    time = nil
    lock.unlock()
  }
}

// MARK: - Core Audio

struct CoreAudioError: LocalizedError {
  let call: String
  let status: OSStatus
  var errorDescription: String? { "\(call) failed (\(status))" }
}

private func check(_ status: OSStatus, _ call: String) throws {
  guard status == noErr else { throw CoreAudioError(call: call, status: status) }
}

private func propertyAddress(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress
{
  AudioObjectPropertyAddress(
    mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal,
    mElement: kAudioObjectPropertyElementMain)
}

extension CallAudioBackend {
  static let coreAudio = CallAudioBackend(
    processes: audioProcesses, open: openProcessTap, watch: watchCallAudio)
}

func audioProcesses() -> [AudioProcess] {
  let system = AudioObjectID(kAudioObjectSystemObject)
  var address = propertyAddress(kAudioHardwarePropertyProcessObjectList)
  var size: UInt32 = 0
  guard AudioObjectGetPropertyDataSize(system, &address, 0, nil, &size) == noErr, size > 0 else {
    return []
  }
  var objects = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
  guard AudioObjectGetPropertyData(system, &address, 0, nil, &size, &objects) == noErr else {
    return []
  }
  return objects.prefix(Int(size) / MemoryLayout<AudioObjectID>.size).compactMap { object in
    var pidAddress = propertyAddress(kAudioProcessPropertyPID)
    var pid: pid_t = 0
    var pidSize = UInt32(MemoryLayout<pid_t>.size)
    guard AudioObjectGetPropertyData(object, &pidAddress, 0, nil, &pidSize, &pid) == noErr else {
      return nil
    }
    var bundleAddress = propertyAddress(kAudioProcessPropertyBundleID)
    var bundle: Unmanaged<CFString>?
    var bundleSize = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    let bundleID =
      AudioObjectGetPropertyData(object, &bundleAddress, 0, nil, &bundleSize, &bundle) == noErr
      ? (bundle?.takeRetainedValue() as String?) ?? "" : ""
    var playingAddress = propertyAddress(kAudioProcessPropertyIsRunningOutput)
    var playing: UInt32 = 0
    var playingSize = UInt32(MemoryLayout<UInt32>.size)
    AudioObjectGetPropertyData(object, &playingAddress, 0, nil, &playingSize, &playing)
    return AudioProcess(object: object, pid: pid, bundleID: bundleID, isPlaying: playing != 0)
  }
}

/// A process tap inside a private aggregate device of its own. The aggregate holds only the tap,
/// not the output device, so switching outputs never removes a device from under it.
private func openProcessTap(
  _ plan: TapPlan, deliver: @escaping (AVAudioPCMBuffer, Double) -> Void,
  formatChanged: @escaping @MainActor () -> Void
) throws -> CallAudioSession {
  let uuid = UUID()
  var tap = AudioObjectID(kAudioObjectUnknown)
  try check(
    AudioHardwareCreateProcessTap(tapDescription(plan, uuid: uuid), &tap),
    "AudioHardwareCreateProcessTap")
  var aggregate = AudioObjectID(kAudioObjectUnknown)
  var procID: AudioDeviceIOProcID?
  let queue = DispatchQueue(label: "so.anarlog.meeting-capture.call-audio", qos: .userInitiated)
  // A Bluetooth headset switching profiles keeps the same output but changes its rate.
  var formatAddress = propertyAddress(kAudioTapPropertyFormat)
  let formatListener: AudioObjectPropertyListenerBlock = { _, _ in
    Task { @MainActor in formatChanged() }
  }
  AudioObjectAddPropertyListenerBlock(tap, &formatAddress, .main, formatListener)
  let close = {
    AudioObjectRemovePropertyListenerBlock(tap, &formatAddress, .main, formatListener)
    if let procID {
      AudioDeviceStop(aggregate, procID)
      AudioDeviceDestroyIOProcID(aggregate, procID)
    }
    if aggregate != kAudioObjectUnknown { AudioHardwareDestroyAggregateDevice(aggregate) }
    AudioHardwareDestroyProcessTap(tap)
  }
  do {
    var description = AudioStreamBasicDescription()
    var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
    try check(
      AudioObjectGetPropertyData(tap, &formatAddress, 0, nil, &size, &description),
      "Reading the tap format")
    guard description.mFormatID == kAudioFormatLinearPCM,
      description.mFormatFlags & kAudioFormatFlagIsFloat != 0, description.mBitsPerChannel == 32,
      description.mChannelsPerFrame > 0, description.mSampleRate > 0,
      let format = AVAudioFormat(
        standardFormatWithSampleRate: description.mSampleRate, channels: 1)
    else { throw CaptureError.invalid("The call audio tap has an unsupported format") }
    let channels = Int(description.mChannelsPerFrame)
    let interleaved = description.mFormatFlags & kAudioFormatFlagIsNonInterleaved == 0

    let aggregateDescription: [String: Any] = [
      kAudioAggregateDeviceNameKey: "Anarlog meeting call audio",
      kAudioAggregateDeviceUIDKey: "so.anarlog.meeting-capture.\(uuid.uuidString)",
      kAudioAggregateDeviceIsPrivateKey: true,
      kAudioAggregateDeviceTapAutoStartKey: true,
      kAudioAggregateDeviceTapListKey: [
        [kAudioSubTapUIDKey: uuid.uuidString, kAudioSubTapDriftCompensationKey: true]
      ],
    ]
    try check(
      AudioHardwareCreateAggregateDevice(aggregateDescription as CFDictionary, &aggregate),
      "AudioHardwareCreateAggregateDevice")
    try check(
      AudioDeviceCreateIOProcIDWithBlock(&procID, aggregate, queue) { _, input, inputTime, _, _ in
        let time = inputTime.pointee
        guard time.mFlags.contains(.hostTimeValid),
          let buffer = downmix(input, channels: channels, interleaved: interleaved, format: format)
        else { return }
        deliver(buffer, AVAudioTime.seconds(forHostTime: time.mHostTime))
      }, "AudioDeviceCreateIOProcIDWithBlock")
    try check(AudioDeviceStart(aggregate, procID), "AudioDeviceStart")
  } catch {
    close()
    throw error
  }
  return CallAudioSession(
    update: { plan in
      // Retarget the running tap in place, keeping its UUID, so a helper that starts playing
      // later joins without a gap.
      var address = propertyAddress(kAudioTapPropertyDescription)
      var description = tapDescription(plan, uuid: uuid)
      return withUnsafeMutablePointer(to: &description) { pointer in
        AudioObjectSetPropertyData(
          tap, &address, 0, nil, UInt32(MemoryLayout<CATapDescription>.size), pointer) == noErr
      }
    }, close: close)
}

/// One mono copy of the tap's buffer. The IO buffer belongs to Core Audio and is reused.
private func downmix(
  _ list: UnsafePointer<AudioBufferList>, channels: Int, interleaved: Bool, format: AVAudioFormat
) -> AVAudioPCMBuffer? {
  let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: list))
  guard let first = buffers.first, let data = first.mData, first.mDataByteSize > 0 else {
    return nil
  }
  let frames = Int(first.mDataByteSize) / MemoryLayout<Float>.size / (interleaved ? channels : 1)
  guard frames > 0, frames <= 65_536,
    let pcm = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(frames)),
    let target = pcm.floatChannelData?[0]
  else { return nil }
  pcm.frameLength = AVAudioFrameCount(frames)
  if channels == 1 {
    target.update(from: data.assumingMemoryBound(to: Float.self), count: frames)
    return pcm
  }
  let scale = 1 / Float(channels)
  for frame in 0..<frames {
    var sum: Float = 0
    for channel in 0..<channels {
      if interleaved {
        sum += data.assumingMemoryBound(to: Float.self)[frame * channels + channel]
      } else if channel < buffers.count, let samples = buffers[channel].mData {
        sum += samples.assumingMemoryBound(to: Float.self)[frame]
      }
    }
    target[frame] = sum * scale
  }
  return pcm
}

/// Listens for a new default output, for processes that start or stop using audio, and for
/// coreaudiod restarting, which destroys every tap. Not the device list: creating the tap's own
/// aggregate device changes it.
private func watchCallAudio(_ changed: @escaping @MainActor (CallAudioChange) -> Void) -> () -> Void
{
  let system = AudioObjectID(kAudioObjectSystemObject)
  let watched: [(AudioObjectPropertySelector, CallAudioChange)] = [
    (kAudioHardwarePropertyDefaultOutputDevice, .output),
    (kAudioHardwarePropertyProcessObjectList, .processes),
    (kAudioHardwarePropertyServiceRestarted, .restarted),
  ]
  let listeners = watched.map { selector, change in
    let listener: AudioObjectPropertyListenerBlock = { _, _ in
      Task { @MainActor in changed(change) }
    }
    var address = propertyAddress(selector)
    AudioObjectAddPropertyListenerBlock(system, &address, .main, listener)
    return (selector, listener)
  }
  return {
    for (selector, listener) in listeners {
      var address = propertyAddress(selector)
      AudioObjectRemovePropertyListenerBlock(system, &address, .main, listener)
    }
  }
}
