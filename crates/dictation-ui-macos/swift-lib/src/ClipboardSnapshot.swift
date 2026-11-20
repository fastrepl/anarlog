import Cocoa

struct ClipboardSnapshot {
  private let items: [[(NSPasteboard.PasteboardType, Data)]]

  init?(_ pasteboard: NSPasteboard) {
    var items: [[(NSPasteboard.PasteboardType, Data)]] = []
    for item in pasteboard.pasteboardItems ?? [] {
      var representations: [(NSPasteboard.PasteboardType, Data)] = []
      for type in item.types {
        guard let data = item.data(forType: type) else { return nil }
        representations.append((type, data))
      }
      items.append(representations)
    }
    self.items = items
  }

  func write(to pasteboard: NSPasteboard) -> Bool {
    // AppKit binds items to a pasteboard after writing; a retry must use fresh objects.
    var copies: [NSPasteboardItem] = []
    for representations in items {
      let copy = NSPasteboardItem()
      for (type, data) in representations {
        guard copy.setData(data, forType: type) else { return false }
      }
      copies.append(copy)
    }
    return copies.isEmpty || pasteboard.writeObjects(copies)
  }
}
