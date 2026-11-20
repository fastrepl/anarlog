import CoreAudio
import Foundation

/// The Mac's default output device, as far as choosing the microphone's processing needs it.
struct OutputRoute: Equatable {
  var name: String
  var uid: String
  var transport: UInt32
  /// The output data source, such as 'hdpn' or 'ispk' on a built-in output that has one.
  var dataSource: UInt32?

  /// The transport as its four-character code ('bltn', 'blue', 'usb ', 'dprt'), for the journal.
  var transportCode: String {
    String(
      bytes: [24, 16, 8, 0].map { UInt8(truncatingIfNeeded: transport >> $0) }, encoding: .ascii)
      ?? String(transport)
  }

  /// A route that puts the sound in the person's ears, so the microphone hears no echo of it.
  ///
  /// Conservative on purpose: whatever this can't place stays on voice processing, which only
  /// costs ducking, while a speaker wrongly taken for headphones would put the call's echo on
  /// the microphone. So only these count:
  /// - Built-in output whose data source is headphones ('hdpn'), or the headphone jack's own
  ///   device, whose UID or name says headphone ("External Headphones" on Apple silicon).
  /// - Bluetooth or USB output named like headphones (AirPods, Beats, Bose QC, Sony WH/WF,
  ///   headset, headphones, earbuds, buds, earphones), unless the name also says speaker
  ///   (Beats Pill, SoundLink, speakerphone, soundbar, HomePod, Sonos).
  /// Having an input doesn't count: speakerphones have microphones too, and once this process
  /// has run voice processing, Core Audio reports an input stream on every output device.
  /// HDMI and DisplayPort monitors, AirPlay, virtual and aggregate devices keep voice processing.
  var isHeadphones: Bool {
    switch transport {
    case kAudioDeviceTransportTypeBuiltIn:
      return dataSource == Self.headphoneDataSource
        || uid.localizedCaseInsensitiveContains("headphone")
        || name.localizedCaseInsensitiveContains("headphone")
    case kAudioDeviceTransportTypeBluetooth, kAudioDeviceTransportTypeBluetoothLE,
      kAudioDeviceTransportTypeUSB:
      return Self.matches(Self.headphoneName, name) && !Self.matches(Self.speakerName, name)
    default:
      return false
    }
  }

  static let headphoneDataSource: UInt32 = 0x6864_706E  // 'hdpn'

  private static let headphoneName = [
    #"airpods|earpods|beats|\bbose\b.*\bqc|quietcomfort|\bw[hf]-"#,
    #"headset|headphone|earbud|\bbuds|earphone"#,
  ].joined(separator: "|")
  private static let speakerName = #"speak|\bpill\b|soundlink|soundbar|homepod|sonos"#

  private static func matches(_ pattern: String, _ name: String) -> Bool {
    name.range(of: pattern, options: [.regularExpression, .caseInsensitive]) != nil
  }

  static func current() -> OutputRoute? {
    guard let device = defaultOutputDevice() else { return nil }
    return OutputRoute(
      name: string(device, kAudioObjectPropertyName) ?? "",
      uid: string(device, kAudioDevicePropertyDeviceUID) ?? "",
      transport: uint32(device, kAudioDevicePropertyTransportType) ?? 0,
      dataSource: uint32(
        device, kAudioDevicePropertyDataSource, scope: kAudioObjectPropertyScopeOutput))
  }

  static func defaultOutputDevice() -> AudioObjectID? {
    uint32(AudioObjectID(kAudioObjectSystemObject), kAudioHardwarePropertyDefaultOutputDevice)
      .flatMap { $0 == kAudioObjectUnknown ? nil : $0 }
  }

  private static func address(
    _ selector: AudioObjectPropertySelector, scope: AudioObjectPropertyScope
  ) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(
      mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain)
  }

  fileprivate static func uint32(
    _ object: AudioObjectID, _ selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal
  ) -> UInt32? {
    var address = address(selector, scope: scope)
    guard AudioObjectHasProperty(object, &address) else { return nil }
    var value: UInt32 = 0
    var size = UInt32(MemoryLayout<UInt32>.size)
    guard AudioObjectGetPropertyData(object, &address, 0, nil, &size, &value) == noErr else {
      return nil
    }
    return value
  }

  fileprivate static func string(_ object: AudioObjectID, _ selector: AudioObjectPropertySelector)
    -> String?
  {
    var address = address(selector, scope: kAudioObjectPropertyScopeGlobal)
    var value: Unmanaged<CFString>?
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    guard AudioObjectGetPropertyData(object, &address, 0, nil, &size, &value) == noErr else {
      return nil
    }
    return value?.takeRetainedValue() as String?
  }
}

/// An input device as the journal names it. Its UID is the one `AVCaptureDevice` reports.
struct InputDevice: Equatable {
  var id: AudioObjectID
  var uid: String
  var name: String
}

extension InputDevice {
  /// The device behind `id`, or nil once it is gone.
  init?(_ id: AudioObjectID) {
    guard id != kAudioObjectUnknown,
      let uid = OutputRoute.string(id, kAudioDevicePropertyDeviceUID)
    else { return nil }
    self.init(id: id, uid: uid, name: OutputRoute.string(id, kAudioObjectPropertyName) ?? "")
  }

  /// The Mac's default input now, which the microphone follows.
  static func current() -> InputDevice? {
    OutputRoute.uint32(
      AudioObjectID(kAudioObjectSystemObject), kAudioHardwarePropertyDefaultInputDevice
    ).flatMap(InputDevice.init)
  }

  /// The devices whose input `device` brings in: itself, or the active sub-devices with input of
  /// an aggregate, such as the private one AVAudioEngine makes of the default input and output.
  static func inputs(of device: AudioObjectID) -> [AudioObjectID] {
    guard
      OutputRoute.uint32(device, kAudioDevicePropertyTransportType)
        == kAudioDeviceTransportTypeAggregate
    else { return [device] }
    var address = AudioObjectPropertyAddress(
      mSelector: kAudioAggregateDevicePropertyActiveSubDeviceList,
      mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == noErr else {
      return []
    }
    var subDevices = [AudioObjectID](
      repeating: kAudioObjectUnknown, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &subDevices) == noErr else {
      return []
    }
    return subDevices.prefix(Int(size) / MemoryLayout<AudioObjectID>.size).filter { subDevice in
      var streams = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyStreams, mScope: kAudioObjectPropertyScopeInput,
        mElement: kAudioObjectPropertyElementMain)
      var size: UInt32 = 0
      return AudioObjectGetPropertyDataSize(subDevice, &streams, 0, nil, &size) == noErr
        && size > 0
    }
  }
}

/// How the microphone is processed for one recording. Only `automatic` is qualified; the others
/// exist so a device trial can measure ducking and echo without a scratch build.
public enum MicrophoneProcessing: String, CaseIterable, Sendable {
  /// Voice processing on speakers, plain on headphones.
  case automatic
  /// Voice processing on every route, ducking other audio at its minimum level.
  case voice
  /// Voice processing on every route, with Apple's advanced ducking, which ducks other audio by
  /// voice activity on top of the level. On built-in speakers it ducked the call by the same
  /// 8 dB as `automatic`.
  case voiceAdvancedDucking = "voice-advanced-ducking"
  /// Never voice processing, so nothing is ducked. On speakers the call's echo stays on the
  /// microphone track and the raw stream; Anarlog's adapter cancels it against the call track.
  case plain

  var advancedDucking: Bool { self == .voiceAdvancedDucking }
}

/// Voice processing cancels the call's echo but ducks every other app's audio, the call included,
/// with no public way to stop it. Headphones leave no echo to cancel; anything else keeps it.
func microphoneUsesVoiceProcessing(
  on route: OutputRoute?, processing: MicrophoneProcessing = .automatic
) -> Bool {
  switch processing {
  case .automatic: return !(route?.isHeadphones ?? false)
  case .voice, .voiceAdvancedDucking: return true
  case .plain: return false
  }
}

/// Calls back on the main actor when the default output, or its data source, changes.
@MainActor
final class OutputRouteWatcher {
  private var listener: AudioObjectPropertyListenerBlock?
  private var watchedDevice: AudioObjectID?
  private var defaultOutput = AudioObjectPropertyAddress(
    mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal,
    mElement: kAudioObjectPropertyElementMain)
  private var dataSource = AudioObjectPropertyAddress(
    mSelector: kAudioDevicePropertyDataSource, mScope: kAudioObjectPropertyScopeOutput,
    mElement: kAudioObjectPropertyElementMain)

  func start(_ changed: @escaping @MainActor () -> Void) {
    guard listener == nil else { return }
    let listener: AudioObjectPropertyListenerBlock = { _, _ in
      Task { @MainActor [weak self] in
        guard let self, self.listener != nil else { return }
        self.followDefaultOutput()
        changed()
      }
    }
    self.listener = listener
    AudioObjectAddPropertyListenerBlock(
      AudioObjectID(kAudioObjectSystemObject), &defaultOutput, .main, listener)
    followDefaultOutput()
  }

  /// A built-in jack that reports plugging as a data source change keeps the same device.
  private func followDefaultOutput() {
    let device = OutputRoute.defaultOutputDevice()
    guard device != watchedDevice, let listener else { return }
    if let watchedDevice {
      AudioObjectRemovePropertyListenerBlock(watchedDevice, &dataSource, .main, listener)
    }
    watchedDevice = nil
    if let device, AudioObjectHasProperty(device, &dataSource),
      AudioObjectAddPropertyListenerBlock(device, &dataSource, .main, listener) == noErr
    {
      watchedDevice = device
    }
  }

  func stop() {
    guard let listener else { return }
    AudioObjectRemovePropertyListenerBlock(
      AudioObjectID(kAudioObjectSystemObject), &defaultOutput, .main, listener)
    if let watchedDevice {
      AudioObjectRemovePropertyListenerBlock(watchedDevice, &dataSource, .main, listener)
    }
    watchedDevice = nil
    self.listener = nil
  }
}
