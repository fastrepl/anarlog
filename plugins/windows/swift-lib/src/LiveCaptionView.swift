import SwiftUI

enum LiveCaptionLayout {
  static let minWidth: CGFloat = 260
  static let defaultWidth: CGFloat = 440
  static let maxWidth: CGFloat = 640
  static let minLineCount = 1
  static let defaultLineCount = 1
  static let maxLineCount = 4
  static let lineHeight: CGFloat = 22
  static let horizontalPadding: CGFloat = 16
  static let verticalPadding: CGFloat = 10
  static let footerHeight: CGFloat = 24
  static let footerGap: CGFloat = 6
  static let cornerRadius: CGFloat = 12
  static let screenMargin: CGFloat = 12
  static let topOffset: CGFloat = 18
  static let minimizedSize = NSSize(width: 42, height: 36)

  static func height(forLineCount lineCount: Int) -> CGFloat {
    let clampedLineCount = min(max(lineCount, minLineCount), maxLineCount)
    return verticalPadding * 2 + lineHeight * CGFloat(clampedLineCount) + footerGap + footerHeight
  }

  static func lineCount(forHeight height: CGFloat) -> Int {
    let textHeight = height - verticalPadding * 2 - footerGap - footerHeight
    let rawLineCount = (textHeight / lineHeight).rounded()
    return min(max(Int(rawLineCount), minLineCount), maxLineCount)
  }
}

struct LiveCaptionView: View {
  @ObservedObject var model: LiveCaptionViewModel
  @ObservedObject var settings: FloatingOverlaySettingsModel
  let onSetMinimized: (Bool) -> Void
  @State private var isHovered = false

  var body: some View {
    Group {
      if settings.liveCaptionMinimized {
        minimizedBody
      } else {
        expandedBody
      }
    }
    .onHover { isHovered = $0 }
  }

  private var expandedBody: some View {
    VStack(spacing: LiveCaptionLayout.footerGap) {
      Text(model.text)
        .font(.system(size: 16, weight: .medium, design: .default))
        .lineSpacing(0)
        .foregroundStyle(.white)
        .multilineTextAlignment(.center)
        .lineLimit(model.lineCount)
        .truncationMode(.head)
        .fixedSize(horizontal: false, vertical: true)
        .frame(
          maxWidth: .infinity,
          minHeight: LiveCaptionLayout.lineHeight * CGFloat(model.lineCount),
          maxHeight: LiveCaptionLayout.lineHeight * CGFloat(model.lineCount),
          alignment: .center
        )

      CaptionFooter(
        opacity: settings.liveCaptionOpacity,
        onSetOpacity: settings.setLiveCaptionOpacity,
        onMinimize: { onSetMinimized(true) }
      )
      .frame(height: LiveCaptionLayout.footerHeight)
    }
    .padding(.horizontal, LiveCaptionLayout.horizontalPadding)
    .padding(.vertical, LiveCaptionLayout.verticalPadding)
    .background(captionBackground)
    .overlay(alignment: .bottomTrailing) {
      ResizeHint()
        .opacity(isHovered ? 0.55 : 0)
        .padding(6)
    }
    .contentShape(RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius))
  }

  private var minimizedBody: some View {
    Button(action: { onSetMinimized(false) }) {
      Image(systemName: "captions.bubble")
        .font(.system(size: 14, weight: .semibold))
        .foregroundStyle(.white)
        .frame(
          width: LiveCaptionLayout.minimizedSize.width,
          height: LiveCaptionLayout.minimizedSize.height
        )
        .background(captionBackground)
    }
    .buttonStyle(.plain)
    .accessibilityLabel("Restore transcript")
  }

  private var captionBackground: some View {
    RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius, style: .continuous)
      .fill(
        Color.black.opacity(
          min(
            max(settings.liveCaptionOpacity, FloatingOverlayOpacity.minLiveCaption),
            FloatingOverlayOpacity.max
          )))
  }
}

private struct CaptionFooter: View {
  let opacity: Double
  let onSetOpacity: (Double) -> Void
  let onMinimize: () -> Void

  var body: some View {
    HStack(spacing: 10) {
      Image(systemName: "circle.lefthalf.filled")
        .font(.system(size: 11, weight: .semibold))
        .foregroundStyle(.white.opacity(0.72))

      Slider(
        value: Binding(get: { clampedOpacity }, set: onSetOpacity),
        in: FloatingOverlayOpacity.minLiveCaption...FloatingOverlayOpacity.max
      )
      .controlSize(.small)
      .accessibilityLabel("Transcript opacity")
      .accessibilityValue("\(Int((clampedOpacity * 100).rounded()))%")

      Button(action: onMinimize) {
        Image(systemName: "minus")
          .font(.system(size: 10, weight: .semibold))
          .foregroundStyle(.white.opacity(0.95))
          .frame(width: 20, height: 20)
          .background(
            Circle()
              .fill(Color.black.opacity(0.58))
              .overlay(
                Circle()
                  .stroke(Color.white.opacity(0.20), lineWidth: 0.5)
              )
              .shadow(color: .black.opacity(0.24), radius: 3, y: 1)
          )
      }
      .buttonStyle(.plain)
      .accessibilityLabel("Minimize transcript")
    }
    .padding(.leading, 8)
    .padding(.trailing, 24)
    .background(
      RoundedRectangle(cornerRadius: 8, style: .continuous)
        .fill(Color.black.opacity(0.42))
        .overlay(
          RoundedRectangle(cornerRadius: 8, style: .continuous)
            .stroke(Color.white.opacity(0.14), lineWidth: 0.5)
        )
    )
  }

  private var clampedOpacity: Double {
    min(max(opacity, FloatingOverlayOpacity.minLiveCaption), FloatingOverlayOpacity.max)
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
