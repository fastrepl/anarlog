import SwiftUI

struct DictationTranscript: View {
  let dictation: FloatingDictationPayload
  let color: Color

  private var emptyText: String {
    if dictation.previewUnavailable {
      return "Live preview is unavailable. Your text will appear when you finish."
    }
    if !dictation.previewEnabled {
      return "Enable Live transcript preview in Settings → Dictation to see words as you speak."
    }
    return dictation.phase == "transcribing"
      ? "Finishing…" : dictation.phase == "starting" ? "Starting…" : "Listening…"
  }

  var body: some View {
    Group {
      if dictation.text.isEmpty && dictation.partial.isEmpty {
        Text(emptyText).foregroundStyle(color.opacity(0.6))
      } else {
        (Text(dictation.text).foregroundColor(color)
          + Text(dictation.text.isEmpty || dictation.partial.isEmpty ? "" : " ")
          + Text(dictation.partial).foregroundColor(color.opacity(0.6)))
      }
    }
    .font(.system(size: 15)).lineSpacing(5)
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.horizontal, 4)
    .fixedSize(horizontal: false, vertical: true)
  }
}
