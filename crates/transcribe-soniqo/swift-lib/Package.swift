// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "soniqo-swift",
  platforms: [.macOS("15.0")],
  products: [
    .library(
      name: "soniqo-swift",
      type: .static,
      targets: ["swift-lib"])
  ],
  dependencies: [
    .package(
      url: "https://github.com/Brendonovich/swift-rs",
      revision: "01980f981bc642a6da382cc0788f18fdd4cde6df"),
    .package(url: "https://github.com/soniqo/speech-swift", exact: "0.0.9"),
  ],
  targets: [
    .target(
      name: "swift-lib",
      dependencies: [
        .product(name: "AudioCommon", package: "speech-swift"),
        .product(name: "OmnilingualASR", package: "speech-swift"),
        .product(name: "ParakeetASR", package: "speech-swift"),
        .product(name: "ParakeetStreamingASR", package: "speech-swift"),
        .product(name: "Qwen3ASR", package: "speech-swift"),
        .product(name: "SwiftRs", package: "swift-rs"),
      ],
      path: "src"
    )
  ]
)
