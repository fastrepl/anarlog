import AudioToolbox
import CoreAudio
import Foundation
import SwiftRs

// The default output device's volume, for ducking other audio during dictation. Input devices
// and routes are never touched.

private func defaultOutputDevice() -> AudioDeviceID? {
  var device = AudioDeviceID(0)
  var size = UInt32(MemoryLayout<AudioDeviceID>.size)
  var address = AudioObjectPropertyAddress(
    mSelector: kAudioHardwarePropertyDefaultOutputDevice,
    mScope: kAudioObjectPropertyScopeGlobal,
    mElement: kAudioObjectPropertyElementMain)
  guard
    AudioObjectGetPropertyData(
      AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device) == noErr,
    device != kAudioObjectUnknown
  else { return nil }
  return device
}

private func deviceUID(_ device: AudioDeviceID) -> String? {
  var uid: Unmanaged<CFString>?
  var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
  var address = AudioObjectPropertyAddress(
    mSelector: kAudioDevicePropertyDeviceUID,
    mScope: kAudioObjectPropertyScopeGlobal,
    mElement: kAudioObjectPropertyElementMain)
  guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &uid) == noErr, let uid
  else { return nil }
  return uid.takeRetainedValue() as String
}

// A headset keeps a separate volume per profile; its sample rate tells them apart.
private func sampleRate(_ device: AudioDeviceID) -> Double? {
  var rate = Float64(0)
  var size = UInt32(MemoryLayout<Float64>.size)
  var address = AudioObjectPropertyAddress(
    mSelector: kAudioDevicePropertyNominalSampleRate,
    mScope: kAudioObjectPropertyScopeGlobal,
    mElement: kAudioObjectPropertyElementMain)
  guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &rate) == noErr else {
    return nil
  }
  return rate
}

private func volumeAddress() -> AudioObjectPropertyAddress {
  AudioObjectPropertyAddress(
    mSelector: kAudioHardwareServiceDeviceProperty_VirtualMainVolume,
    mScope: kAudioDevicePropertyScopeOutput,
    mElement: kAudioObjectPropertyElementMain)
}

private func isMuted(_ device: AudioDeviceID) -> Bool {
  var muted = UInt32(0)
  var size = UInt32(MemoryLayout<UInt32>.size)
  var address = AudioObjectPropertyAddress(
    mSelector: kAudioDevicePropertyMute,
    mScope: kAudioDevicePropertyScopeOutput,
    mElement: kAudioObjectPropertyElementMain)
  return AudioObjectHasProperty(device, &address)
    && AudioObjectGetPropertyData(device, &address, 0, nil, &size, &muted) == noErr && muted != 0
}

/// `{"device", "profile", "volume"}` for the default output; volume is absent while muted or
/// without a settable volume. "" when there is no default output.
@_cdecl("_dictation_output_level")
public func dictationOutputLevel() -> SRString {
  guard let device = defaultOutputDevice(), let uid = deviceUID(device) else { return SRString("") }
  var level: [String: Any] = ["device": uid]
  if let rate = sampleRate(device) { level["profile"] = String(Int(rate)) }
  var address = volumeAddress()
  var settable = DarwinBoolean(false)
  var volume = Float32(0)
  var size = UInt32(MemoryLayout<Float32>.size)
  if !isMuted(device), AudioObjectHasProperty(device, &address),
    AudioObjectIsPropertySettable(device, &address, &settable) == noErr, settable.boolValue,
    AudioObjectGetPropertyData(device, &address, 0, nil, &size, &volume) == noErr
  {
    level["volume"] = Double(volume)
  }
  guard let data = try? JSONSerialization.data(withJSONObject: level) else { return SRString("") }
  return SRString(String(decoding: data, as: UTF8.self))
}

/// Sets the default output's volume only while `device` is still the default output.
@_cdecl("_dictation_set_output_level")
public func dictationSetOutputLevel(device: SRString, volume: Float) -> Bool {
  guard let current = defaultOutputDevice(), deviceUID(current) == device.toString() else {
    return false
  }
  var address = volumeAddress()
  var value = Float32(min(max(volume, 0), 1))
  return AudioObjectSetPropertyData(
    current, &address, 0, nil, UInt32(MemoryLayout<Float32>.size), &value) == noErr
}
