import Foundation
import SwiftRs

@_cdecl("_show_dictation_overlay")
public func _showDictationOverlay() -> Bool {
  OverlayManager.shared.show()
  return true
}

@_cdecl("_hide_dictation_overlay")
public func _hideDictationOverlay() -> Bool {
  OverlayManager.shared.hide()
  return true
}

@_cdecl("_update_dictation_state")
public func _updateDictationState(json: SRString) -> Bool {
  let jsonString = json.toString()
  guard let data = jsonString.data(using: .utf8),
    let payload = try? JSONDecoder().decode(DictationStatePayload.self, from: data)
  else {
    return false
  }
  OverlayManager.shared.update(state: payload)
  return true
}
