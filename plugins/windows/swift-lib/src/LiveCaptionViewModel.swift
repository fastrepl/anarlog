import Combine
import Foundation

final class LiveCaptionViewModel: ObservableObject {
  @Published var text: String = ""
  @Published var lineCount: Int = LiveCaptionLayout.defaultLineCount
}
