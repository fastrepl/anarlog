import CoreText
import Foundation

enum FloatingBarFonts {
  static let cabinSketchName = "CabinSketch-Regular"

  private static var didRegister = false

  static func register() {
    guard !didRegister else { return }
    didRegister = true

    guard let url = cabinSketchURL() else {
      return
    }

    CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
  }

  private static func cabinSketchURL() -> URL? {
    let fileManager = FileManager.default
    let resourceName = "CabinSketch-Regular"

    let candidates = [
      Bundle.main.url(forResource: resourceName, withExtension: "ttf"),
      Bundle.main.resourceURL?.appendingPathComponent("\(resourceName).ttf"),
      Bundle.main.resourceURL?.appendingPathComponent(
        "windows-swift_swift-lib.bundle/\(resourceName).ttf"),
      URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("Resources/\(resourceName).ttf"),
    ]

    return candidates.compactMap { $0 }.first {
      fileManager.fileExists(atPath: $0.path)
    }
  }
}
