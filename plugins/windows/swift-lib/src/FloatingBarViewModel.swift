import Combine
import Foundation

final class FloatingBarViewModel: ObservableObject {
  @Published var amplitude: Double = 0
  @Published var status: FloatingBarStatus = .recording
  @Published var colorScheme: FloatingBarColorScheme = .dark
  @Published var isExpanded: Bool = false
  @Published var liveCaptionToggleVisible: Bool = false
  @Published var title: String = "Live transcript"
  @Published var transcriptBubbles: [FloatingTranscriptBubblePayload] = []
}
