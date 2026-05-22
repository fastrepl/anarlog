import Foundation

@_silgen_name("rust_on_floating_bar_stop")
private func rustOnFloatingBarStop()

@_silgen_name("rust_on_floating_bar_open_main")
private func rustOnFloatingBarOpenMain()

enum RustBridge {
  static func stopListening() {
    rustOnFloatingBarStop()
  }

  static func openMainWindow() {
    rustOnFloatingBarOpenMain()
  }
}
