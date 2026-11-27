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
      revision: "207e4bfe5d36fdd0cc08f28d1849ff94ac7e24b8")
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
    ),
    .testTarget(
      name: "swift-lib-tests",
      dependencies: ["swift-lib"],
      path: "tests"
    ),
  ]
)
