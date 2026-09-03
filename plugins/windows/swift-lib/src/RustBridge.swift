import Foundation

@_silgen_name("rust_on_floating_bar_stop")
private func rustOnFloatingBarStop()

@_silgen_name("rust_on_floating_bar_open_main")
private func rustOnFloatingBarOpenMain()

@_silgen_name("rust_on_floating_bar_settings_change")
private func rustOnFloatingBarSettingsChange(_ settingsPtr: UnsafePointer<CChar>)

enum RustBridge {
  static func stopListening() {
    rustOnFloatingBarStop()
  }

  static func openMainWindow() {
    rustOnFloatingBarOpenMain()
  }

  static func floatingBarSettingsChanged(_ payload: FloatingOverlaySettingsChangePayload) {
    guard
      let data = try? JSONEncoder().encode(payload),
      let json = String(data: data, encoding: .utf8)
    else {
      return
    }

    json.withCString { settingsPtr in
      rustOnFloatingBarSettingsChange(settingsPtr)
    }
  }
}
