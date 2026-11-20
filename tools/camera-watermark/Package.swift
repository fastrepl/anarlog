// swift-tools-version: 5.9

import PackageDescription

// Anarlog Camera: a Camera Extension that shows the person's camera with an Anarlog badge while
// Anarlog records. `scripts/bundle.mjs` assembles the two executables into `Anarlog Camera.app`,
// which holds the system extension and feeds it. See README.md and PROVENANCE.md.
let package = Package(
  name: "AnarlogCamera",
  platforms: [.macOS("14.0")],
  products: [
    .executable(name: "anarlog-camera", targets: ["WatermarkHelper"]),
    .executable(name: "anarlog-camera-extension", targets: ["WatermarkExtension"]),
  ],
  targets: [
    .target(name: "WatermarkKit"),
    .executableTarget(name: "WatermarkHelper", dependencies: ["WatermarkKit"]),
    .executableTarget(name: "WatermarkExtension", dependencies: ["WatermarkKit"]),
    .testTarget(name: "WatermarkKitTests", dependencies: ["WatermarkKit"]),
  ]
)
