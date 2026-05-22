import CoreText
import Foundation

enum FloatingBarFonts {
  static let cabinSketchName = "CabinSketch-Regular"

  private static var didRegister = false

  static func register() {
    guard !didRegister else { return }
    didRegister = true

    guard
      let url = Bundle.module.url(
        forResource: "CabinSketch-Regular",
        withExtension: "ttf")
    else {
      return
    }

    CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
  }
}
