// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "windows-swift",
  platforms: [.macOS("14.2")],
  products: [
    .library(
      name: "windows-swift",
      type: .static,
      targets: ["swift-lib"])
  ],
  dependencies: [
    .package(
      url: "https://github.com/Brendonovich/swift-rs",
      revision: "01980f981bc642a6da382cc0788f18fdd4cde6df")
  ],
  targets: [
    .target(
      name: "swift-lib",
      dependencies: [
        .product(name: "SwiftRs", package: "swift-rs")
      ],
      path: "src",
      resources: [
        .process("Resources")
      ]
    )
  ]
)
