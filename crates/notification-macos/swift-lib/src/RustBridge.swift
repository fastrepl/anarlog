import Foundation

@_silgen_name("rust_on_collapsed_confirm")
private func rustOnCollapsedConfirm(_ keyPtr: UnsafePointer<CChar>, _ tag: Int32)

@_silgen_name("rust_on_expanded_accept")
private func rustOnExpandedAccept(_ keyPtr: UnsafePointer<CChar>, _ tag: Int32)

@_silgen_name("rust_on_dismiss")
private func rustOnDismiss(_ keyPtr: UnsafePointer<CChar>, _ tag: Int32)

@_silgen_name("rust_on_collapsed_timeout")
private func rustOnCollapsedTimeout(_ keyPtr: UnsafePointer<CChar>, _ tag: Int32)

@_silgen_name("rust_on_expanded_start_time_reached")
private func rustOnExpandedStartTimeReached(_ keyPtr: UnsafePointer<CChar>, _ tag: Int32)

@_silgen_name("rust_on_option_selected")
private func rustOnOptionSelected(_ keyPtr: UnsafePointer<CChar>, _ tag: Int32)

enum RustBridge {
  static func onCollapsedConfirm(key: String) {
    key.withCString { keyPtr in
      rustOnCollapsedConfirm(keyPtr, -1)
    }
  }

  static func onExpandedAccept(key: String) {
    key.withCString { keyPtr in
      rustOnExpandedAccept(keyPtr, -1)
    }
  }

  static func onDismiss(key: String) {
    key.withCString { keyPtr in
      rustOnDismiss(keyPtr, -1)
    }
  }

  static func onCollapsedTimeout(key: String) {
    key.withCString { keyPtr in
      rustOnCollapsedTimeout(keyPtr, -1)
    }
  }

  static func onExpandedStartTimeReached(key: String) {
    key.withCString { keyPtr in
      rustOnExpandedStartTimeReached(keyPtr, -1)
    }
  }

  static func onOptionSelected(key: String, selectedIndex: Int32) {
    key.withCString { keyPtr in
      rustOnOptionSelected(keyPtr, selectedIndex)
    }
  }
}
