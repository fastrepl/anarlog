import Cocoa

@_silgen_name("rust_set_force_quit")
func rustSetForceQuit()

@_cdecl("_setup_force_quit_handler")
public func _setupForceQuitHandler() {
  QuitInterceptor.shared.setup()
}

@_cdecl("_show_quit_overlay")
public func _showQuitOverlay() {
  DispatchQueue.main.async {
    QuitInterceptor.shared.showOverlay()
  }
}

@_cdecl("_demo_quit_progress")
public func _demoQuitProgress() {
  DispatchQueue.main.async {
    QuitInterceptor.shared.showOverlay()
  }
}
