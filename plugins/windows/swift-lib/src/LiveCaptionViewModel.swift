import Combine
import Foundation

final class LiveCaptionViewModel: ObservableObject {
  @Published var text: String = ""
  @Published var opacity: Double = 0.78
  @Published var lineCount: Int = LiveCaptionLayout.defaultLineCount
}
