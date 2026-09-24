// swift-tools-version:5.9

import Foundation
import PackageDescription

// The Zoom Meeting SDK is proprietary and never vendored. Point ANARLOG_ZOOM_SDK_PATH at an
// unpacked `zoom-sdk-macos-*/ZoomSDK` directory to build the real client; without it the
// package builds a stub sidecar that speaks the same bridge protocol and fails fast on `start`.
let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let zoomSDKPath =
  ProcessInfo.processInfo.environment["ANARLOG_ZOOM_SDK_PATH"]
  ?? packageDirectory.appendingPathComponent("Vendor/ZoomSDK").path
let hasZoomSDK = FileManager.default.fileExists(
  atPath: zoomSDKPath + "/ZoomSDK.framework/Headers/ZoomSDK.h")

var mainDependencies: [Target.Dependency] = ["ZoomBridgeProtocol"]
var extraTargets: [Target] = []
var mainSwiftSettings: [SwiftSetting] = []

if hasZoomSDK {
  mainDependencies.append("ZoomSDKShim")
  mainSwiftSettings.append(.define("ANARLOG_ZOOM_SDK"))
  extraTargets.append(
    .target(
      name: "ZoomSDKShim",
      path: "Sources/ZoomSDKShim",
      publicHeadersPath: "include",
      cSettings: [.unsafeFlags(["-fobjc-arc", "-F", zoomSDKPath])],
      linkerSettings: [
        .linkedFramework("ZoomSDK"),
        .linkedFramework("AppKit"),
        .unsafeFlags([
          "-F", zoomSDKPath,
          "-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks",
          "-Xlinker", "-rpath", "-Xlinker", zoomSDKPath,
        ]),
      ]
    ))
}

let package = Package(
  name: "char-sidecar-zoom",
  platforms: [.macOS("14.2")],
  products: [
    .executable(name: "char-sidecar-zoom", targets: ["ZoomSidecar"])
  ],
  targets: [
    .target(
      name: "ZoomBridgeProtocol",
      path: "Sources/ZoomBridgeProtocol"
    ),
    .executableTarget(
      name: "ZoomSidecar",
      dependencies: mainDependencies,
      path: "Sources/ZoomSidecar",
      swiftSettings: mainSwiftSettings
    ),
    .testTarget(
      name: "ZoomBridgeProtocolTests",
      dependencies: ["ZoomBridgeProtocol"],
      path: "Tests/ZoomBridgeProtocolTests"
    ),
  ] + extraTargets
)
