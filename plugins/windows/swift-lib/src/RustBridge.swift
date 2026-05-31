import Foundation

@_silgen_name("rust_on_floating_bar_stop")
private func rustOnFloatingBarStop()

@_silgen_name("rust_on_floating_bar_open_main")
private func rustOnFloatingBarOpenMain()

@_silgen_name("rust_on_devtools_panel_action")
private func rustOnDevtoolsPanelAction(_ actionPtr: UnsafePointer<CChar>)

enum RustBridge {
  static func stopListening() {
    rustOnFloatingBarStop()
  }

  static func openMainWindow() {
    rustOnFloatingBarOpenMain()
  }

  static func devtoolsPanelAction(_ action: String) {
    action.withCString { actionPtr in
      rustOnDevtoolsPanelAction(actionPtr)
    }
  }
}
