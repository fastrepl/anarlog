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
  static let footerHeight: CGFloat = 32
  static let footerSeparatorHeight: CGFloat = 1
  static let cornerRadius: CGFloat = 12
  static let screenMargin: CGFloat = 12
  static let topOffset: CGFloat = 18

  static func height(forLineCount lineCount: Int) -> CGFloat {
    let clampedLineCount = min(max(lineCount, minLineCount), maxLineCount)
    return verticalPadding * 2 + lineHeight * CGFloat(clampedLineCount) + footerSeparatorHeight
      + footerHeight
  }

  static func lineCount(forHeight height: CGFloat) -> Int {
    let textHeight = height - verticalPadding * 2 - footerSeparatorHeight - footerHeight
    let rawLineCount = (textHeight / lineHeight).rounded()
    return min(max(Int(rawLineCount), minLineCount), maxLineCount)
  }
}

struct LiveCaptionView: View {
  @ObservedObject var model: LiveCaptionViewModel
  @ObservedObject var settings: FloatingOverlaySettingsModel
  let onSetMinimized: (Bool) -> Void

  var body: some View {
    VStack(spacing: 0) {
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
        .padding(.horizontal, LiveCaptionLayout.horizontalPadding)
        .padding(.vertical, LiveCaptionLayout.verticalPadding)

      Rectangle()
        .fill(Color.white.opacity(0.16))
        .frame(height: LiveCaptionLayout.footerSeparatorHeight)

      CaptionFooter(
        opacity: settings.liveCaptionOpacity,
        onSetOpacity: settings.setLiveCaptionOpacity,
        onMinimize: { onSetMinimized(true) }
      )
      .frame(height: LiveCaptionLayout.footerHeight)
    }
    .background(captionBackground)
    .contentShape(RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius))
  }

  private var captionBackground: some View {
    RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius, style: .continuous)
      .fill(
        Color.black.opacity(
          min(
            max(settings.liveCaptionOpacity, FloatingOverlayOpacity.minLiveCaption),
            FloatingOverlayOpacity.maxLiveCaption
          )))
  }
}

private struct CaptionFooter: View {
  private let sliderWidth: CGFloat = 120
  let opacity: Double
  let onSetOpacity: (Double) -> Void
  let onMinimize: () -> Void

  var body: some View {
    HStack(spacing: 8) {
      Slider(
        value: Binding(get: { clampedOpacity }, set: onSetOpacity),
        in: FloatingOverlayOpacity.minLiveCaption...FloatingOverlayOpacity.maxLiveCaption
      )
      .controlSize(.small)
      .frame(width: sliderWidth)
      .accessibilityLabel("Transcript opacity")
      .accessibilityValue("\(Int((clampedOpacity * 100).rounded()))%")

      Spacer(minLength: 0)

      Button(action: onMinimize) {
        Text("Hide")
          .font(.system(size: 11, weight: .semibold))
          .foregroundStyle(.white.opacity(0.92))
          .padding(.horizontal, 8)
          .frame(height: 20)
          .background(
            Capsule(style: .continuous)
              .fill(Color.black.opacity(0.42))
              .overlay(
                Capsule(style: .continuous)
                  .stroke(Color.white.opacity(0.18), lineWidth: 0.5)
              )
          )
      }
      .buttonStyle(.plain)
      .accessibilityLabel("Hide transcript")
    }
    .padding(.leading, LiveCaptionLayout.horizontalPadding)
    .padding(.trailing, 8)
  }

  private var clampedOpacity: Double {
    min(max(opacity, FloatingOverlayOpacity.minLiveCaption), FloatingOverlayOpacity.maxLiveCaption)
  }
}
