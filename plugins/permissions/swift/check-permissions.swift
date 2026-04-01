import AVFoundation
import Contacts
import EventKit
import Foundation

guard CommandLine.arguments.count > 1 else {
  fputs(
    "Usage: check-permissions <calendar|contacts|microphone|systemAudio|accessibility>\n", stderr)
  exit(1)
}

let permissionType = CommandLine.arguments[1]

switch permissionType {
case "calendar":
  switch EKEventStore.authorizationStatus(for: .event) {
  case .notDetermined: print("notDetermined")
  case .restricted: print("restricted")
  case .denied: print("denied")
  case .fullAccess: print("fullAccess")
  case .writeOnly: print("writeOnly")
  @unknown default: print("unknown")
  }
case "contacts":
  switch CNContactStore.authorizationStatus(for: .contacts) {
  case .notDetermined: print("notDetermined")
  case .restricted: print("restricted")
  case .denied: print("denied")
  case .authorized: print("authorized")
  @unknown default: print("unknown")
  }
case "microphone":
  switch AVCaptureDevice.authorizationStatus(for: .audio) {
  case .notDetermined: print("notDetermined")
  case .restricted: print("restricted")
  case .denied: print("denied")
  case .authorized: print("authorized")
  @unknown default: print("unknown")
  }
case "systemAudio":
  let TCC_PATH = "/System/Library/PrivateFrameworks/TCC.framework/Versions/A/TCC"
  guard let handle = dlopen(TCC_PATH, RTLD_NOW),
    let sym = dlsym(handle, "TCCAccessPreflight")
  else {
    print("error")
    exit(1)
  }
  typealias PreflightFunc = @convention(c) (CFString, CFDictionary?) -> Int
  let preflight = unsafeBitCast(sym, to: PreflightFunc.self)
  let result = preflight("kTCCServiceAudioCapture" as CFString, nil)
  switch result {
  case 0: print("authorized")
  case 1: print("denied")
  case 2: print("notDetermined")
  default: print("unknown")
  }
case "accessibility":
  print(AXIsProcessTrusted() ? "trusted" : "untrusted")
default:
  fputs("Unknown permission type: \(permissionType)\n", stderr)
  exit(1)
}
