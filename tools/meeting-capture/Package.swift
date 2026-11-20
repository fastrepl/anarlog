// swift-tools-version: 5.9

import Foundation
import PackageDescription

let infoPlist = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
  .appendingPathComponent("Info.plist").path

let package = Package(
  name: "AnarlogMeetingCapture",
  platforms: [.macOS("15.0")],
  products: [.executable(name: "anarlog-meeting-capture", targets: ["CaptureCLI"])],
  targets: [
    // Catches the Objective-C exceptions AVAudioEngine raises, which would abort the helper.
    .target(
      name: "AudioEngineGuard",
      cSettings: [.unsafeFlags(["-fobjc-arc", "-fobjc-arc-exceptions"])]),
    .target(name: "CaptureCore", dependencies: ["AudioEngineGuard"]),
    .target(name: "MeetingAccessibility", dependencies: ["CaptureCore"]),
    .executableTarget(
      name: "CaptureCLI", dependencies: ["CaptureCore", "MeetingAccessibility"],
      linkerSettings: [
        .unsafeFlags([
          "-Xlinker", "-sectcreate", "-Xlinker", "__TEXT", "-Xlinker", "__info_plist",
          "-Xlinker", infoPlist,
        ])
      ]),
    .testTarget(name: "CaptureCoreTests", dependencies: ["CaptureCore", "AudioEngineGuard"]),
    .testTarget(name: "MeetingAccessibilityTests", dependencies: ["MeetingAccessibility"]),
  ]
)
