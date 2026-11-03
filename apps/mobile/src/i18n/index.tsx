import AsyncStorage from "@react-native-async-storage/async-storage";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createContext, useContext, type ReactNode } from "react";
import { Alert, Text as NativeText, type TextProps } from "react-native";

export type AppLanguage = "vi" | "en";
const LANGUAGE_KEY = "anarlog.mobile.language";
let activeLanguage: AppLanguage = "vi";
const LanguageContext = createContext<{
  language: AppLanguage;
  setLanguage: (language: AppLanguage) => void;
}>({
  language: "vi",
  setLanguage: () => {},
});

const vi: Record<string, string> = {
  "Something went wrong": "Đã xảy ra lỗi",
  "Your notes are still stored on this device.":
    "Ghi chú của bạn vẫn được lưu trên thiết bị này.",
  "Try again": "Thử lại",
  Settings: "Cài đặt",
  Account: "Tài khoản",
  "Sync & storage": "Đồng bộ và lưu trữ",
  Recording: "Ghi âm",
  "Transcription & summaries": "Chép lời và tóm tắt",
  Appearance: "Giao diện",
  Privacy: "Quyền riêng tư",
  "Help & about": "Trợ giúp và giới thiệu",
  Theme: "Chủ đề",
  System: "Theo hệ thống",
  Light: "Sáng",
  Dark: "Tối",
  "Notes list": "Danh sách ghi chú",
  "Show folder": "Hiện thư mục",
  "Show tags": "Hiện thẻ",
  "Loading meetings…": "Đang tải cuộc họp…",
  "Couldn't load meetings": "Không tải được cuộc họp",
  "No meetings yet": "Chưa có cuộc họp nào",
  "Start listening, import a recording, or create a note.":
    "Bắt đầu ghi âm, nhập bản ghi hoặc tạo ghi chú.",
  "Retry loading meetings": "Thử tải lại cuộc họp",
  "Load more meetings": "Tải thêm cuộc họp",
  "Import recording": "Nhập bản ghi âm",
  "Import meeting": "Nhập cuộc họp",
  "Start listening": "Bắt đầu ghi âm",
  "Search meetings": "Tìm cuộc họp",
  "Open settings": "Mở cài đặt",
  "Create new note": "Tạo ghi chú mới",
  "Couldn’t import recording": "Không thể nhập bản ghi âm",
  "Choose another audio file and try again.":
    "Hãy chọn tệp âm thanh khác rồi thử lại.",
  "Couldn’t import meeting": "Không thể nhập cuộc họp",
  "Choose a valid meeting package.": "Hãy chọn gói cuộc họp hợp lệ.",
  Delete: "Xóa",
  Untitled: "Chưa có tiêu đề",
  Language: "Ngôn ngữ",
  Vietnamese: "Tiếng Việt",
  English: "Tiếng Anh",
  Back: "Quay lại",
  Done: "Xong",
  Cancel: "Hủy",
  Save: "Lưu",
  "Search by title or note content": "Tìm theo tiêu đề hoặc nội dung ghi chú",
  "No matches": "Không tìm thấy kết quả",
  "Clear search": "Xóa nội dung tìm kiếm",
  "New note": "Ghi chú mới",
  Search: "Tìm kiếm",
  Close: "Đóng",
  Edit: "Chỉnh sửa",
  Retry: "Thử lại",
  Stop: "Dừng",
  Continue: "Tiếp tục",
  "Saved recordings": "Bản ghi đã lưu",
  "No saved recordings": "Chưa có bản ghi nào",
  "On this device": "Trên thiết bị này",
  "Not downloaded": "Chưa tải xuống",
  "Help center": "Trung tâm trợ giúp",
  "Contact support": "Liên hệ hỗ trợ",
  "Privacy policy": "Chính sách quyền riêng tư",
  "Terms of service": "Điều khoản dịch vụ",
  "Action Button": "Nút Tác vụ",
  "Start listening in one press": "Bắt đầu ghi âm chỉ với một lần nhấn",
  "Allow microphone access": "Cho phép truy cập micrô",
  "Allow Microphone": "Cho phép micrô",
  "Open Settings": "Mở Cài đặt",
  "Assign Start Listening": "Gán thao tác Bắt đầu ghi âm",
  "Try it": "Dùng thử",
  "Long press": "Nhấn giữ",
  Listen: "Ghi âm",
  "Recording not on this phone": "Bản ghi không có trên điện thoại này",
  Files: "Tệp",
  "Last used": "Dùng gần đây",
  "More with Anarlog Pro": "Khám phá thêm với Anarlog Pro",
  "Transcribing…": "Đang chép lời…",
  "Speaking…": "Đang nói…",
  "Save transcript edit": "Lưu chỉnh sửa bản chép lời",
  "Opening your notes…": "Đang mở ghi chú…",
  "Could not unlock. Please try again.": "Không thể mở khóa. Vui lòng thử lại.",
  "No sync activity yet.": "Chưa có hoạt động đồng bộ.",
  "Loading…": "Đang tải…",
  Provider: "Nhà cung cấp",
  "Choose a configured provider to select a model.":
    "Chọn nhà cung cấp đã thiết lập để chọn mô hình.",
  Model: "Mô hình",
  Automatic: "Tự động",
  "Main language": "Ngôn ngữ chính",
  "Model ready for offline transcription.":
    "Mô hình sẵn sàng chép lời ngoại tuyến.",
  "Listen with the Action Button": "Ghi âm bằng Nút Tác vụ",
  Folder: "Thư mục",
  Export: "Xuất",
  "Version history": "Lịch sử phiên bản",
  "Meeting glossary": "Bảng thuật ngữ cuộc họp",
  "Loading devices…": "Đang tải thiết bị…",
  "Your dictionary is empty.": "Từ điển của bạn đang trống.",
  "This development build works locally without an account.":
    "Bản dựng phát triển này có thể hoạt động cục bộ mà không cần tài khoản.",
};

Object.assign(vi, {
  "Add a name, acronym, or term": "Thêm tên, từ viết tắt hoặc thuật ngữ",
  "Add names, products, and technical terms for this meeting, one per line. Transcription uses these as recognition hints; it does not replace words in the transcript.":
    "Thêm tên người, sản phẩm và thuật ngữ chuyên ngành cho cuộc họp này, mỗi dòng một mục. Tính năng chép lời dùng các mục này làm gợi ý nhận dạng, không thay thế từ ngữ trong bản chép lời.",
  "Add term": "Thêm thuật ngữ",
  "Additional spoken languages": "Ngôn ngữ nói bổ sung",
  "After your trial, your notes and recordings stay on this device. Subscribe to keep sync and Anarlog models, or use your own API keys.":
    "Sau thời gian dùng thử, ghi chú và bản ghi âm vẫn được lưu trên thiết bị này. Đăng ký để tiếp tục đồng bộ và dùng các mô hình của Anarlog, hoặc sử dụng API key của riêng bạn.",
  "Anarlog is listening": "Anarlog đang ghi âm",
  "Approve access in your browser, then paste the authorization code here.":
    "Phê duyệt quyền truy cập trong trình duyệt, sau đó dán mã ủy quyền vào đây.",
  "Attach file": "Đính kèm tệp",
  "Audio downloads stay on this device. Cloud recordings can be downloaded from each note.":
    "Tệp âm thanh tải xuống được lưu trên thiết bị này. Bạn có thể tải bản ghi trên đám mây từ từng ghi chú.",
  "Authorization code": "Mã ủy quyền",
  Bold: "In đậm",
  "Bulleted list": "Danh sách dấu đầu dòng",
  "By signing up, you agree to our": "Khi đăng ký, bạn đồng ý với",
  "Cancel download": "Hủy tải xuống",
  Checklist: "Danh sách kiểm tra",
  "Choose file instead": "Chọn tệp khác",
  "Close folder picker": "Đóng bộ chọn thư mục",
  "Close note actions": "Đóng thao tác ghi chú",
  "Close version history": "Đóng lịch sử phiên bản",
  "Cloud sync": "Đồng bộ đám mây",
  "Configure Action Button": "Thiết lập Nút Tác vụ",
  "Configure providers": "Thiết lập nhà cung cấp",
  "Configure this provider below to load models.":
    "Thiết lập nhà cung cấp bên dưới để tải danh sách mô hình.",
  "Connected devices": "Thiết bị đã kết nối",
  "Could not complete this change. Please try again.":
    "Không thể hoàn tất thay đổi này. Vui lòng thử lại.",
  "Couldn’t load this meeting’s glossary. Close and try again.":
    "Không thể tải bảng thuật ngữ cuộc họp này. Hãy đóng rồi thử lại.",
  "Couldn’t update this note. Try again.":
    "Không thể cập nhật ghi chú này. Hãy thử lại.",
  "Create “": "Tạo “",
  Dictionary: "Từ điển",
  Disconnect: "Ngắt kết nối",
  "Dismiss note actions": "Đóng thao tác ghi chú",
  "Earlier versions of this note appear here as you edit it.":
    "Các phiên bản trước của ghi chú sẽ xuất hiện tại đây khi bạn chỉnh sửa.",
  "Edit transcript": "Chỉnh sửa bản chép lời",
  "Edited on another device at the same time. The later edit was kept.":
    "Ghi chú được chỉnh sửa đồng thời trên thiết bị khác. Thay đổi mới hơn đã được giữ lại.",
  Email: "Email",
  "Enable device code login in ChatGPT Settings → Security if prompted.":
    "Nếu được yêu cầu, hãy bật đăng nhập bằng mã thiết bị trong ChatGPT → Cài đặt → Bảo mật.",
  "Enter model ID…": "Nhập mã mô hình…",
  "Error reports": "Báo cáo lỗi",
  "Explore Anarlog Pro": "Khám phá Anarlog Pro",
  "Formatted note — edit the body on desktop":
    "Ghi chú đã định dạng — chỉnh sửa nội dung trên máy tính",
  "Get started": "Bắt đầu",
  Heading: "Tiêu đề",
  "Help improve Anarlog": "Giúp cải thiện Anarlog",
  "Help transcription recognize teammate names, company jargon, and product terms. Up to 100 terms, 100 characters each.":
    "Giúp tính năng chép lời nhận diện tên đồng đội, thuật ngữ nội bộ và tên sản phẩm. Tối đa 100 mục, mỗi mục 100 ký tự.",
  "Hide keyboard": "Ẩn bàn phím",
  "HTTPS base URL": "URL gốc HTTPS",
  "Last successful sync": "Lần đồng bộ thành công gần nhất",
  "Long-press once to start a meeting recording and again to stop.":
    "Nhấn giữ một lần để bắt đầu ghi âm cuộc họp, rồi nhấn giữ lần nữa để dừng.",
  "Manage account & subscription": "Quản lý tài khoản và gói đăng ký",
  "Meeting glossary terms": "Thuật ngữ cuộc họp",
  Microphone: "Micrô",
  "Model ID": "Mã mô hình",
  "More actions": "Thao tác khác",
  "Note title": "Tiêu đề ghi chú",
  "Off by default. Recordings are saved on this device and transcribed after you stop. Turn this on to stream audio to the selected live transcription provider while recording.":
    "Mặc định tắt. Bản ghi được lưu trên thiết bị và chép lời sau khi bạn dừng. Bật tùy chọn này để truyền âm thanh đến nhà cung cấp chép lời trực tiếp đã chọn trong lúc ghi âm.",
  "Open Kimi Code": "Mở Kimi Code",
  "Open the note to keep the version on this device or use the other one.":
    "Mở ghi chú để giữ phiên bản trên thiết bị này hoặc chọn phiên bản còn lại.",
  Plan: "Gói dịch vụ",
  "Privacy Policy": "Chính sách quyền riêng tư",
  "Recent sync status changes since this app opened, newest first. This history resets when the app restarts. Note contents and credentials are not included.":
    "Các thay đổi trạng thái đồng bộ từ khi mở ứng dụng, mới nhất ở trên. Lịch sử sẽ đặt lại khi khởi động lại ứng dụng. Nội dung ghi chú và thông tin đăng nhập không được lưu ở đây.",
  "Recording ended unexpectedly. The saved audio includes everything captured before the interruption.":
    "Ghi âm kết thúc đột ngột. Tệp âm thanh đã lưu bao gồm mọi nội dung thu được trước khi bị gián đoạn.",
  "Recording meeting": "Đang ghi âm cuộc họp",
  "Recording storage": "Dung lượng bản ghi",
  Recordings: "Bản ghi âm",
  "Recordings are saved on this device first. Keep Anarlog open to finish backing them up.":
    "Bản ghi âm được lưu trên thiết bị này trước. Hãy để Anarlog mở để hoàn tất sao lưu.",
  "Refresh plan": "Làm mới gói dịch vụ",
  Remove: "Xóa",
  "Remove key": "Xóa khóa",
  Restore: "Khôi phục",
  "Retry now": "Thử lại ngay",
  "Retry trial activation": "Thử kích hoạt lại thời gian dùng thử",
  "Search or create folder": "Tìm hoặc tạo thư mục",
  "Select model": "Chọn mô hình",
  "Select provider": "Chọn nhà cung cấp",
  "Share anonymous usage and sanitized error reports. Notes, transcripts, recordings, and API keys are excluded.":
    "Chia sẻ dữ liệu sử dụng ẩn danh và báo cáo lỗi đã loại bỏ thông tin nhận dạng. Ghi chú, bản chép lời, bản ghi âm và API key sẽ không được chia sẻ.",
  "Showing the first 50 matches. Narrow your search to find more.":
    "Đang hiển thị 50 kết quả đầu tiên. Hãy thu hẹp tìm kiếm để tìm thêm.",
  "Sign in again": "Đăng nhập lại",
  "Spoken languages": "Ngôn ngữ được nói",
  "Start typing…": "Bắt đầu nhập…",
  "Structured meeting minutes with decisions, action items, and transcript evidence. Each generation sends your Anarlog session token, notes, and transcript text to this URL. Audio is never sent. Use only a Worker you trust; the app asks before every request.":
    "Tạo biên bản cuộc họp có cấu trúc gồm quyết định, việc cần làm và dẫn chứng từ bản chép lời. Mỗi lần tạo sẽ gửi mã phiên Anarlog, ghi chú và nội dung bản chép lời đến URL này. Âm thanh không bao giờ được gửi. Chỉ dùng Worker mà bạn tin cậy; ứng dụng sẽ hỏi trước mỗi lần gửi.",
  "Summary provider": "Nhà cung cấp tóm tắt",
  "Sync across your devices and use Anarlog models for transcription and summaries. New users get a free two-week trial, shared with desktop.":
    "Đồng bộ giữa các thiết bị và dùng mô hình Anarlog để chép lời, tóm tắt. Người dùng mới được dùng thử miễn phí hai tuần, dùng chung với ứng dụng máy tính.",
  "Sync health": "Tình trạng đồng bộ",
  "Sync log": "Nhật ký đồng bộ",
  "Tap the selected folder to remove this note from it.":
    "Nhấn vào thư mục đã chọn để gỡ ghi chú này khỏi thư mục.",
  "Terms of Service": "Điều khoản dịch vụ",
  "The AI notepad for": "Sổ ghi chép AI dành cho",
  "The main language guides transcription and is used for summaries.":
    "Ngôn ngữ chính được dùng để định hướng chép lời và tạo bản tóm tắt.",
  "The original recognition and each edit are retained in this meeting’s transcript history.":
    "Kết quả nhận dạng ban đầu và mọi chỉnh sửa đều được lưu trong lịch sử bản chép lời của cuộc họp này.",
  "Transcribe live while recording": "Chép lời trực tiếp khi ghi âm",
  "Transcript edited since this summary was generated. Regenerate it to include the latest changes.":
    "Bản chép lời đã được chỉnh sửa sau khi tạo tóm tắt. Hãy tạo lại để đưa các thay đổi mới nhất vào.",
  "Transcript segment text": "Nội dung đoạn chép lời",
  "Transcription provider": "Nhà cung cấp chép lời",
  "Trial:": "Dùng thử:",
  Unlock: "Mở khóa",
  "Usage analytics": "Phân tích mức sử dụng",
  "Use other version": "Dùng phiên bản khác",
  "Use the API key from your Kimi Code membership settings.":
    "Dùng API key trong phần cài đặt thành viên Kimi Code.",
  "Valid settings save automatically. API keys are stored locally and sent to their provider when used. Meeting text is not sent automatically after transcription; Generate summary asks you to confirm the selected destination first. Anarlog Pro and a Cloudflare Worker also receive your Anarlog session token to verify your account.":
    "Cài đặt hợp lệ được lưu tự động. API key được lưu trên thiết bị và chỉ gửi đến nhà cung cấp khi sử dụng. Nội dung cuộc họp không tự động được gửi sau khi chép lời; khi tạo tóm tắt, ứng dụng sẽ yêu cầu bạn xác nhận nơi nhận trước. Anarlog Pro và Cloudflare Worker cũng nhận mã phiên Anarlog để xác minh tài khoản.",
  "Waiting for approval… After approving, return here to finish connecting.":
    "Đang chờ phê duyệt… Sau khi phê duyệt, quay lại đây để hoàn tất kết nối.",
  "Waiting for note content to sync…": "Đang chờ nội dung ghi chú đồng bộ…",
  "Your iPhone Action Button can start a new Anarlog recording or stop the one already in progress.":
    "Nút Tác vụ trên iPhone có thể bắt đầu bản ghi Anarlog mới hoặc dừng bản ghi đang chạy.",
  "Dismisses the notification": "Đóng thông báo",
  "Open recording settings": "Mở cài đặt ghi âm",
  "Recover recording": "Khôi phục bản ghi",
  "Stop listening": "Dừng ghi âm",
  "Your plan changed. Sync and model access are still updating.":
    "Gói dịch vụ của bạn đã thay đổi. Quyền đồng bộ và truy cập mô hình đang được cập nhật.",
  and: "và",
  "days left": "ngày còn lại",
  "private meetings.": "các cuộc họp riêng tư.",
  "Loading Anarlog": "Đang tải Anarlog",
  "Loading models…": "Đang tải mô hình…",
  "Lock app": "Khóa ứng dụng",
  Italic: "In nghiêng",
  Keep: "Giữ lại",
  Version: "Phiên bản",
  "Evidence unavailable": "Không có dẫn chứng",
  "This transcript segment is no longer available.":
    "Đoạn bản chép lời này không còn khả dụng.",
  "Couldn’t attach file": "Không thể đính kèm tệp",
  "The selected file could not be attached.": "Không thể đính kèm tệp đã chọn.",
  "Couldn’t export meeting": "Không thể xuất cuộc họp",
  "Try again.": "Hãy thử lại.",
  "Export meeting": "Xuất cuộc họp",
  "Choose whether to include the recording. Audio stays on this device unless you share the exported package.":
    "Chọn có đưa bản ghi âm vào hay không. Âm thanh vẫn ở trên thiết bị này trừ khi bạn chia sẻ gói đã xuất.",
  "The recording is not available on this device. The package will contain meeting text only.":
    "Bản ghi âm không có trên thiết bị này. Gói xuất chỉ chứa nội dung cuộc họp.",
  "Without audio": "Không kèm âm thanh",
  "Include audio": "Kèm âm thanh",
  "Your notes and transcript text will be sent to":
    "Ghi chú và bản chép lời sẽ được gửi đến",
  "for this request. Audio is not sent.":
    "cho yêu cầu này. Âm thanh sẽ không được gửi.",
  "Continue only if you trust this destination.":
    "Chỉ tiếp tục nếu bạn tin cậy nơi nhận này.",
  "Send and generate": "Gửi và tạo",
  "Your personal notes and recordings stay on this device. Sync will connect them to this account and bring in its existing notes. Copies in your previous account remain there. Team and shared notes stay with their workspace. Reconnect integrations for this account. After connecting, this library requires an app version that supports account switching.":
    "Ghi chú và bản ghi cá nhân vẫn ở trên thiết bị này. Tính năng đồng bộ sẽ liên kết chúng với tài khoản này và tải các ghi chú hiện có của tài khoản về. Bản sao trong tài khoản trước vẫn được giữ ở đó. Ghi chú nhóm và ghi chú được chia sẻ vẫn thuộc không gian làm việc tương ứng. Hãy kết nối lại các tích hợp cho tài khoản này. Sau khi kết nối, thư viện này cần phiên bản ứng dụng có hỗ trợ chuyển đổi tài khoản.",
  "Keep using locally": "Tiếp tục dùng cục bộ",
  "Connect library": "Kết nối thư viện",
  Today: "Hôm nay",
  Tomorrow: "Ngày mai",
  Yesterday: "Hôm qua",
  "Date unavailable": "Không có ngày",
  "date unavailable": "không có ngày",
});

Object.assign(vi, {
  "Google Drive backup": "Sao lưu Google Drive",
  "Google Drive backup is currently available on iPhone only.":
    "Hiện có thể dùng tính năng sao lưu Google Drive trên iPhone.",
  "Connected Google account": "Tài khoản Google đã kết nối",
  "Google Drive not connected": "Chưa kết nối Google Drive",
  "Back up now": "Sao lưu ngay",
  "Creating backup…": "Đang tạo bản sao lưu…",
  "R2 backup": "Sao lưu R2",
  "Automatic backup": "Sao lưu tự động",
  "Sign in required": "Cần đăng nhập",
  "Anarlog Pro required": "Cần gói Anarlog Pro",
  "Enabled for this account": "Đã bật cho tài khoản này",
  "Encrypted notes, transcripts, app data, and audio recordings are backed up to your private R2 bucket while Anarlog is open.":
    "Ghi chú, bản chép lời, dữ liệu ứng dụng và bản ghi âm được sao lưu mã hóa vào vùng lưu trữ R2 riêng tư khi Anarlog đang mở.",
  "Restore latest backup": "Khôi phục bản sao lưu mới nhất",
  "Preparing restore…": "Đang chuẩn bị khôi phục…",
  "Disconnect Google Drive": "Ngắt kết nối Google Drive",
  "Disconnecting…": "Đang ngắt kết nối…",
  "Backup downloaded. Force close and reopen Anarlog to apply it.":
    "Đã tải bản sao lưu. Hãy buộc đóng rồi mở lại Anarlog để áp dụng.",
  "Backup saved to Google Drive.": "Đã lưu bản sao lưu vào Google Drive.",
  "Connect Google Drive": "Kết nối Google Drive",
  "Google Drive is not configured yet. Add the iOS OAuth Client ID and rebuild the app.":
    "Google Drive chưa được cấu hình. Hãy thêm OAuth Client ID cho iOS rồi dựng lại ứng dụng.",
  "Google sign-in was cancelled.": "Đã hủy đăng nhập Google.",
  "Google sign-in returned an invalid response.":
    "Google trả về phản hồi đăng nhập không hợp lệ.",
  "Google sign-in did not return an authorization code.":
    "Google không trả về mã cấp quyền.",
  "Google could not finish connecting this account.":
    "Không thể hoàn tất kết nối tài khoản Google.",
  "Google did not return the account email.":
    "Google không trả về địa chỉ email tài khoản.",
  "Could not upload the backup to Google Drive.":
    "Không thể tải bản sao lưu lên Google Drive.",
  "No Anarlog backup was found in this Google account.":
    "Không tìm thấy bản sao lưu Anarlog trong tài khoản Google này.",
  "Could not download the Google Drive backup.":
    "Không thể tải bản sao lưu từ Google Drive.",
  "Could not access Anarlog backups in Google Drive.":
    "Không thể truy cập bản sao lưu Anarlog trên Google Drive.",
  "Connect Google Drive first.": "Hãy kết nối Google Drive trước.",
  "Google Drive access expired. Disconnect and connect it again.":
    "Quyền truy cập Google Drive đã hết hạn. Hãy ngắt kết nối rồi kết nối lại.",
  "Restore will replace this iPhone’s local database when Anarlog next starts. Continue?":
    "Bản sao lưu sẽ thay thế cơ sở dữ liệu cục bộ trên iPhone này khi Anarlog khởi động lại. Tiếp tục?",
  "Database backups are encrypted on this device. The encryption key is kept in this iPhone’s Keychain, so database restore currently requires this same device. Audio files use Anarlog’s end-to-end encrypted attachment backup.":
    "Cơ sở dữ liệu được mã hóa ngay trên thiết bị. Khóa mã hóa được giữ trong Keychain của iPhone này nên hiện chỉ có thể khôi phục cơ sở dữ liệu trên chính thiết bị này. Tệp ghi âm dùng cơ chế sao lưu tệp đính kèm mã hóa đầu cuối của Anarlog.",
  "Could not upload the encrypted R2 backup.":
    "Không thể tải bản sao lưu R2 đã mã hóa lên.",
  "Could not download the R2 backup.": "Không thể tải bản sao lưu R2 xuống.",
  "No R2 backup was found.": "Không tìm thấy bản sao lưu R2.",
  "R2 backup service is unavailable.":
    "Dịch vụ sao lưu R2 hiện không khả dụng.",
  "R2 backup service returned an invalid upload URL.":
    "Dịch vụ sao lưu R2 trả về URL tải lên không hợp lệ.",
  "Sign in to restore your backup.": "Đăng nhập để khôi phục bản sao lưu.",
  "Restore backup": "Khôi phục bản sao lưu",
});

Object.assign(vi, {
  Memos: "Ghi chú",
  Transcript: "Bản chép lời",
  Summary: "Tóm tắt",
  "Save glossary": "Lưu bảng thuật ngữ",
  "Long press or swipe left for actions":
    "Nhấn giữ hoặc vuốt sang trái để mở thao tác",
  "After the meeting, review the transcript and tap Generate summary. Anarlog will ask before sending meeting text to your selected provider.":
    "Sau cuộc họp, hãy xem lại bản chép lời rồi nhấn Tạo tóm tắt. Anarlog sẽ hỏi trước khi gửi nội dung cuộc họp đến nhà cung cấp bạn đã chọn.",
  "Anarlog can start listening as soon as it opens.":
    "Anarlog có thể bắt đầu ghi âm ngay khi mở ứng dụng.",
  "Anarlog has the meeting details, but the recording has not synced yet.":
    "Anarlog đã có thông tin cuộc họp nhưng bản ghi âm chưa được đồng bộ.",
  "Anarlog needs the microphone to record when you press the button.":
    "Anarlog cần quyền truy cập micrô để ghi âm khi bạn nhấn nút.",
  "Back up encrypted audio": "Sao lưu âm thanh đã mã hóa",
  "Check connection": "Kiểm tra kết nối",
  "Checking model…": "Đang kiểm tra mô hình…",
  "Checking your free Pro trial. You can start taking notes and recording now.":
    "Đang kiểm tra thời gian dùng thử Pro miễn phí. Bạn có thể bắt đầu ghi chú và ghi âm ngay.",
  "Checking…": "Đang kiểm tra…",
  "Choose recording": "Chọn bản ghi âm",
  "Choose summary provider": "Chọn nhà cung cấp tóm tắt",
  "Choose transcription provider": "Chọn nhà cung cấp chép lời",
  "Choose your model here. Configure connections below.":
    "Chọn mô hình tại đây. Thiết lập kết nối bên dưới.",
  "Cloud sync and Anarlog models are included in your subscription.":
    "Gói đăng ký của bạn bao gồm đồng bộ đám mây và các mô hình Anarlog.",
  "Cloud sync is available during your Pro trial and with a Pro subscription. Your local notes and recordings are still available.":
    "Bạn có thể dùng đồng bộ đám mây trong thời gian dùng thử Pro và khi đăng ký Pro. Ghi chú và bản ghi âm cục bộ vẫn khả dụng.",
  "Cloud sync is off": "Đồng bộ đám mây đang tắt",
  "Connect this local library": "Kết nối thư viện cục bộ này",
  "Connected accounts": "Tài khoản đã kết nối",
  "Connecting…": "Đang kết nối…",
  "Could not load recordings.": "Không tải được bản ghi âm.",
  "Could not restart the backup. Try again.":
    "Không thể khởi động lại quá trình sao lưu. Hãy thử lại.",
  "Could not verify plan": "Không thể xác minh gói dịch vụ",
  "Couldn’t load earlier versions. Close and try again.":
    "Không tải được các phiên bản trước. Hãy đóng rồi thử lại.",
  "Couldn’t load folders. Close and try again.":
    "Không tải được thư mục. Hãy đóng rồi thử lại.",
  "Couldn’t load models. You can still enter a model ID.":
    "Không tải được mô hình. Bạn vẫn có thể nhập mã mô hình.",
  "Couldn't load the transcript. Reopen the transcript to retry.":
    "Không tải được bản chép lời. Hãy mở lại để thử lần nữa.",
  "Couldn’t move this note. Try again.":
    "Không thể di chuyển ghi chú này. Hãy thử lại.",
  "Couldn’t restore this version. Try again.":
    "Không thể khôi phục phiên bản này. Hãy thử lại.",
  "Couldn’t save this glossary.": "Không thể lưu bảng thuật ngữ.",
  "Couldn’t save this transcript edit.":
    "Không thể lưu chỉnh sửa bản chép lời.",
  "Couldn't search meetings. Try again.":
    "Không thể tìm cuộc họp. Hãy thử lại.",
  "Download model": "Tải mô hình xuống",
  "Download recording": "Tải bản ghi âm xuống",
  "Download the encrypted recording from your Anarlog sync.":
    "Tải bản ghi âm đã mã hóa từ dữ liệu đồng bộ Anarlog của bạn.",
  "Edited · Edit": "Đã chỉnh sửa · Sửa",
  "Empty note": "Ghi chú trống",
  "Enter a valid folder name.": "Nhập tên thư mục hợp lệ.",
  "Finish connecting": "Hoàn tất kết nối",
  "Finishing transcription. When it completes, review the transcript and tap Generate summary to continue.":
    "Đang hoàn tất chép lời. Khi xong, hãy xem lại bản chép lời rồi nhấn Tạo tóm tắt để tiếp tục.",
  "Generate summary": "Tạo tóm tắt",
  "Generating summary…": "Đang tạo tóm tắt…",
  "Hide sync log": "Ẩn nhật ký đồng bộ",
  "Included with your Pro trial or subscription. No API key needed.":
    "Đã bao gồm trong thời gian dùng thử Pro hoặc gói đăng ký. Không cần API key.",
  "Loading transcript…": "Đang tải bản chép lời…",
  "Local dev": "Phát triển cục bộ",
  "No completed sync in this session":
    "Chưa có lần đồng bộ nào hoàn tất trong phiên này",
  "No folders found.": "Không tìm thấy thư mục.",
  "No folders yet. Enter a name to create one.":
    "Chưa có thư mục. Nhập tên để tạo thư mục mới.",
  "No models found. You can enter a model ID.":
    "Không tìm thấy mô hình. Bạn có thể nhập mã mô hình.",
  "No transcript yet.": "Chưa có bản chép lời.",
  "Not signed in": "Chưa đăng nhập",
  "Notes and recording are free. Cloud sync and Anarlog models require an active Pro trial or subscription. You can also use your own API keys.":
    "Ghi chú và ghi âm miễn phí. Đồng bộ đám mây và mô hình Anarlog cần thời gian dùng thử Pro hoặc gói đăng ký còn hiệu lực. Bạn cũng có thể dùng API key của riêng mình.",
  "Opening connected accounts…": "Đang mở các tài khoản đã kết nối…",
  "Preparing backup…": "Đang chuẩn bị sao lưu…",
  "Press and hold the Action Button. Anarlog opens and starts listening; press and hold again to stop.":
    "Nhấn giữ Nút Tác vụ để mở Anarlog và bắt đầu ghi âm; nhấn giữ lần nữa để dừng.",
  "Refresh access": "Làm mới quyền truy cập",
  "Refreshing…": "Đang làm mới…",
  "Regenerate summary": "Tạo lại bản tóm tắt",
  "Require Face ID, fingerprint, or your device passcode to open Anarlog. Recording continues while the app is locked.":
    "Yêu cầu Face ID, vân tay hoặc mật mã thiết bị để mở Anarlog. Ứng dụng vẫn tiếp tục ghi âm khi bị khóa.",
  "Retry summary": "Thử tạo lại tóm tắt",
  "Retry transcription": "Thử chép lời lại",
  "Saved locally · retrying when online":
    "Đã lưu cục bộ · sẽ thử lại khi có mạng",
  "Saving Worker URL…": "Đang lưu URL Worker…",
  "Saving…": "Đang lưu…",
  "Searching…": "Đang tìm kiếm…",
  "Set up a device passcode to lock Anarlog.":
    "Thiết lập mật mã thiết bị để khóa Anarlog.",
  "Sign out": "Đăng xuất",
  "Signing out…": "Đang đăng xuất…",
  "Start Listening works. Press and hold the Action Button to start listening, and again to stop.":
    "Thao tác Bắt đầu ghi âm đã sẵn sàng. Nhấn giữ Nút Tác vụ để bắt đầu ghi âm và nhấn giữ lần nữa để dừng.",
  "Start or stop listening in one press":
    "Bắt đầu hoặc dừng ghi âm chỉ với một lần nhấn",
  "Starting sign-in…": "Đang bắt đầu đăng nhập…",
  "Sync now": "Đồng bộ ngay",
  "Tap to transcribe": "Chạm để chép lời",
  "Transcribes after recording.": "Chép lời sau khi ghi âm.",
  "Transcribes live while recording.": "Chép lời trực tiếp trong khi ghi âm.",
  "Transcription failed — tap to retry":
    "Chép lời không thành công — chạm để thử lại",
  "Unnamed device": "Thiết bị chưa có tên",
  "Verifying key…": "Đang xác minh khóa…",
  "View plans": "Xem các gói",
  "View sync log": "Xem nhật ký đồng bộ",
  "Your meeting summary will appear here. Your memos are in the Memos tab.":
    "Bản tóm tắt cuộc họp sẽ xuất hiện tại đây. Ghi chú nằm trong thẻ Ghi chú.",
  "Your notes stay on this device. Turn on cloud sync to keep them end-to-end encrypted across your devices.":
    "Ghi chú vẫn được lưu trên thiết bị này. Bật đồng bộ đám mây để mã hóa đầu cuối và đồng bộ giữa các thiết bị.",
  "Your Pro trial includes cloud sync and Anarlog models. After it ends, you can keep using your notes, recording, and your own API keys.":
    "Thời gian dùng thử Pro bao gồm đồng bộ đám mây và các mô hình Anarlog. Sau khi hết hạn, bạn vẫn có thể dùng ghi chú, ghi âm và API key của riêng mình.",
  "Your recording is saved. Finish transcription to get your summary.":
    "Bản ghi âm đã được lưu. Hoàn tất chép lời để nhận bản tóm tắt.",
  "Your recording will be transcribed after you stop listening.":
    "Bản ghi sẽ được chép lời sau khi bạn dừng ghi âm.",
  "Your transcript will appear here as you speak.":
    "Bản chép lời sẽ xuất hiện tại đây khi bạn nói.",
});

export function translate(value: string, language: AppLanguage): string {
  if (language !== "vi") return value;
  if (vi[value]) return vi[value];
  if (value === "now") return "vừa xong";
  const relative = value.match(
    /^(in )?(\d+) (min|mins|hour|hours|day|days|month|months|year|years)( ago)?$/,
  );
  if (relative) {
    const unit = relative[3].startsWith("min")
      ? "phút"
      : relative[3].startsWith("hour")
        ? "giờ"
        : relative[3].startsWith("day")
          ? "ngày"
          : relative[3].startsWith("month")
            ? "tháng"
            : "năm";
    return relative[1]
      ? `${relative[2]} ${unit} nữa`
      : relative[4]
        ? `${relative[2]} ${unit} trước`
        : `${relative[2]} ${unit}`;
  }
  const terms = value.match(/^(\d+) terms?$/);
  if (terms) return `${terms[1]} thuật ngữ`;
  const slots = value.match(/^(\d+) of (\d+) device slots in use$/);
  if (slots) return `Đang dùng ${slots[1]}/${slots[2]} vị trí thiết bị`;
  const conflicts = value.match(
    /^(\d+) notes? (has|have) a version from another device$/,
  );
  if (conflicts) return `${conflicts[1]} ghi chú có phiên bản từ thiết bị khác`;
  const waiting = value.match(
    /^(\d+) records? (is|are) waiting for an app update$/,
  );
  if (waiting) return `${waiting[1]} bản ghi đang chờ bản cập nhật ứng dụng`;
  const tooLarge = value.match(/^(\d+) records? (is|are) too large to apply$/);
  if (tooLarge) return `${tooLarge[1]} bản ghi quá lớn để áp dụng`;
  const deleteLabel = value.match(/^Delete (.+)$/);
  if (deleteLabel) return `Xóa ${deleteLabel[1]}`;
  const completedLabel = value.match(/^(.+), done$/);
  if (completedLabel) {
    return `${translate(completedLabel[1], language)}, đã xong`;
  }
  const editSegment = value.match(/^Edit transcript segment, (.+)$/);
  if (editSegment) return `Chỉnh sửa đoạn chép lời, ${editSegment[1]}`;
  const downloadModel = value.match(/^Downloading… (\d+)%$/);
  if (downloadModel) return `Đang tải xuống… ${downloadModel[1]}%`;
  const providerStatus = value.match(/^(.+) · (Active|Connected|Key saved)$/);
  if (providerStatus) {
    const statuses: Record<string, string> = {
      Active: "Đang dùng",
      Connected: "Đã kết nối",
      "Key saved": "Đã lưu khóa",
    };
    return `${providerStatus[1]} · ${statuses[providerStatus[2]]}`;
  }
  const subscription = value.match(
    /^Use your (.+) subscription for summaries\. Sign in once on this device\.$/,
  );
  if (subscription)
    return `Dùng gói đăng ký ${subscription[1]} để tạo tóm tắt. Chỉ cần đăng nhập một lần trên thiết bị này.`;
  const authCode = value.match(/^Enter this code in (.+): (.+)$/);
  if (authCode) return `Nhập mã này trong ${authCode[1]}: ${authCode[2]}`;
  const openProvider = value.match(/^Open (.+)$/);
  if (openProvider) return `Mở ${openProvider[1]}`;
  const connectedProvider = value.match(
    /^(.+) connected\. Choose it as your summary provider above\.$/,
  );
  if (connectedProvider)
    return `Đã kết nối ${connectedProvider[1]}. Hãy chọn nhà cung cấp này để tóm tắt ở trên.`;
  const syncDescription = value.match(
    /^(.+) · (Synced|Syncing…|Waiting to back up|Saved on this device|Not backed up)$/,
  );
  if (syncDescription) {
    const statuses: Record<string, string> = {
      Synced: "Đã đồng bộ",
      "Syncing…": "Đang đồng bộ…",
      "Waiting to back up": "Đang chờ sao lưu",
      "Saved on this device": "Đã lưu trên thiết bị này",
      "Not backed up": "Chưa sao lưu",
    };
    return `${syncDescription[1]} · ${statuses[syncDescription[2]]}`;
  }
  const deletion = value.match(/^Delete "(.+)"\?$/);
  if (deletion) {
    const title = deletion[1] === "Untitled" ? "Chưa có tiêu đề" : deletion[1];
    return `Xóa “${title}”?`;
  }
  const sync = value.match(/^Sync this local library with (.+)\?$/);
  if (sync) return `Đồng bộ thư viện cục bộ này với ${sync[1]}?`;
  const send = value.match(/^Send meeting text to (.+)\?$/);
  if (send) return `Gửi nội dung cuộc họp đến ${send[1]}?`;
  const destination = value.match(
    /^Your notes and transcript text will be sent to (.+) for this request\. Audio is not sent\.(.*)$/,
  );
  if (destination) {
    const suffix = destination[2]
      .replace(
        " Your Anarlog session token will also be sent there to verify your account.",
        " Mã phiên Anarlog của bạn cũng sẽ được gửi đến đó để xác minh tài khoản.",
      )
      .replace(
        " Continue only if you trust this destination.",
        " Chỉ tiếp tục nếu bạn tin cậy nơi nhận này.",
      );
    return `Ghi chú và bản chép lời sẽ được gửi đến ${destination[1]} cho yêu cầu này. Âm thanh sẽ không được gửi.${suffix}`;
  }
  const workerDownload = value.match(
    /^Downloads (.+) \((\d+) MB\) to this device\. Audio and transcription stay local\. A summary provider receives transcript text only when you tap Generate summary\.$/,
  );
  if (workerDownload)
    return `Tải ${workerDownload[1]} (${workerDownload[2]} MB) xuống thiết bị này. Âm thanh và bản chép lời được xử lý cục bộ. Nhà cung cấp tóm tắt chỉ nhận nội dung bản chép lời khi bạn nhấn Tạo tóm tắt.`;
  return value;
}

export function localizedAlert(...args: Parameters<typeof Alert.alert>) {
  const [title, message, buttons, options] = args;
  Alert.alert(
    translate(title, activeLanguage),
    message ? translate(message, activeLanguage) : message,
    buttons?.map((button) => ({
      ...button,
      text: button.text ? translate(button.text, activeLanguage) : undefined,
    })),
    options,
  );
}

export function getAppLanguage() {
  return activeLanguage;
}

export function LanguageProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const languageQuery = useQuery({
    queryKey: [LANGUAGE_KEY],
    queryFn: async (): Promise<AppLanguage> => {
      const value = await AsyncStorage.getItem(LANGUAGE_KEY);
      const language = value === "en" ? "en" : "vi";
      activeLanguage = language;
      return language;
    },
    placeholderData: "vi" as AppLanguage,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const language = languageQuery.data ?? "vi";
  const setLanguage = (next: AppLanguage) => {
    activeLanguage = next;
    queryClient.setQueryData([LANGUAGE_KEY], next);
    void AsyncStorage.setItem(LANGUAGE_KEY, next);
  };
  return (
    <LanguageContext.Provider value={{ language, setLanguage }}>
      {children}
    </LanguageContext.Provider>
  );
}

export function useLanguage() {
  return useContext(LanguageContext);
}

export function Text({ children, ...props }: TextProps) {
  const { language } = useLanguage();
  const localized =
    typeof children === "string" ? translate(children, language) : children;
  return <NativeText {...props}>{localized}</NativeText>;
}
