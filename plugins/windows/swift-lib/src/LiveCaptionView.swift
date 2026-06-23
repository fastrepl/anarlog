import SwiftUI

enum LiveCaptionLayout {
  static let minWidth: CGFloat = 260
  static let defaultWidth: CGFloat = 440
  static let maxWidth: CGFloat = 440
  static let minLineCount = 1
  static let defaultLineCount = 1
  static let maxLineCount = 4
  static let lineHeight: CGFloat = 22
  static let horizontalPadding: CGFloat = 16
  static let verticalPadding: CGFloat = 10
  static let cornerRadius: CGFloat = 12
  static let screenMargin: CGFloat = 12
  static let topOffset: CGFloat = 18

  static func height(forLineCount lineCount: Int) -> CGFloat {
    let clampedLineCount = min(max(lineCount, minLineCount), maxLineCount)
    return verticalPadding * 2 + lineHeight * CGFloat(clampedLineCount)
  }

  static func lineCount(forHeight height: CGFloat) -> Int {
    let rawLineCount = ((height - verticalPadding * 2) / lineHeight).rounded()
    return min(max(Int(rawLineCount), minLineCount), maxLineCount)
  }
}

struct LiveCaptionView: View {
  @ObservedObject var model: LiveCaptionViewModel
  @State private var isHovered = false

  var body: some View {
    Text(model.text)
      .font(.system(size: 16, weight: .medium, design: .default))
      .lineSpacing(0)
      .foregroundStyle(.white)
      .lineLimit(model.lineCount)
      .truncationMode(.tail)
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
      .padding(.horizontal, LiveCaptionLayout.horizontalPadding)
      .padding(.vertical, LiveCaptionLayout.verticalPadding)
      .background(
        RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius, style: .continuous)
          .fill(Color.black.opacity(min(max(model.opacity, 0.35), 0.95)))
      )
      .overlay(alignment: .bottomTrailing) {
        ResizeHint()
          .opacity(isHovered ? 0.55 : 0)
          .padding(6)
      }
      .contentShape(RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius))
      .onHover { isHovered = $0 }
  }
}

private struct ResizeHint: View {
  var body: some View {
    VStack(alignment: .trailing, spacing: 2) {
      Capsule()
        .fill(.white)
        .frame(width: 6, height: 1)
      Capsule()
        .fill(.white)
        .frame(width: 10, height: 1)
    }
    .accessibilityHidden(true)
  }
}
