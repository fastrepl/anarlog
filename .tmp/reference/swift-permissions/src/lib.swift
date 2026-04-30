import Foundation

private let grantedValue = 0
private let deniedValue = 1
private let neverRequestedValue = 2
private let errorValue = -1

private let tccPath = "/System/Library/PrivateFrameworks/TCC.framework/Versions/A/TCC"

private let tccHandle: UnsafeMutableRawPointer? = {
  dlopen(tccPath, RTLD_NOW)
}()

private typealias TCCPreflightFunc = @convention(c) (CFString, CFDictionary?) -> Int

@_cdecl("_audio_capture_permission_status")
public func _audio_capture_permission_status() -> Int {
  guard let tccHandle,
    let functionSymbol = dlsym(tccHandle, "TCCAccessPreflight"),
    let preflight = unsafeBitCast(functionSymbol, to: TCCPreflightFunc.self) as TCCPreflightFunc?
  else {
    return errorValue
  }

  return preflight("kTCCServiceAudioCapture" as CFString, nil)
}
