import AVFoundation
import Contacts
import EventKit
import Foundation

guard CommandLine.arguments.count > 1 else {
  fputs("Usage: check-permissions <calendar|contacts|microphone|accessibility>\n", stderr)
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
case "accessibility":
  print(AXIsProcessTrusted() ? "trusted" : "untrusted")
default:
  fputs("Unknown permission type: \(permissionType)\n", stderr)
  exit(1)
}
