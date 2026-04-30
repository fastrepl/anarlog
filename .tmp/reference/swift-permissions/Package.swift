// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "permissions-swift",
  platforms: [.macOS("15.0")],
  products: [
    .library(
      name: "permissions-swift",
      type: .static,
      targets: ["PermissionsSwift"])
  ],
  dependencies: [
    .package(url: "https://github.com/Brendonovich/swift-rs", exact: "1.0.7"),
    .package(url: "https://github.com/soniqo/speech-swift", exact: "0.0.9"),
  ],
  targets: [
    .target(
      name: "PermissionsSwift",
      dependencies: [
        .product(name: "AudioCommon", package: "speech-swift"),
        .product(name: "OmnilingualASR", package: "speech-swift"),
        .product(name: "ParakeetASR", package: "speech-swift"),
        .product(name: "ParakeetStreamingASR", package: "speech-swift"),
        .product(name: "Qwen3ASR", package: "speech-swift"),
        .product(name: "SpeechVAD", package: "speech-swift"),
        .product(name: "SwiftRs", package: "swift-rs"),
      ],
      path: "src")
  ]
)
