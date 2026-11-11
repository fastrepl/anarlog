// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "soniqo-swift",
  platforms: [.macOS("14.2")],
  products: [
    .library(
      name: "soniqo-swift",
      type: .static,
      targets: ["swift-lib"])
  ],
  dependencies: [
    .package(
      url: "https://github.com/Brendonovich/swift-rs",
      revision: "207e4bfe5d36fdd0cc08f28d1849ff94ac7e24b8"),
    .package(url: "https://github.com/soniqo/speech-swift", exact: "0.0.27"),
  ],
  targets: [
    .target(
      name: "swift-lib",
      dependencies: [
        .product(name: "AudioCommon", package: "speech-swift"),
        .product(name: "OmnilingualASR", package: "speech-swift"),
        .product(name: "ParakeetASR", package: "speech-swift"),
        .product(name: "ParakeetStreamingASR", package: "speech-swift"),
        .product(name: "SpeechVAD", package: "speech-swift"),
        .product(name: "SwiftRs", package: "swift-rs"),
      ],
      path: "src"
    )
  ]
)
