package so.anarlog.localspeech

import android.os.ParcelFileDescriptor
import kotlinx.coroutines.flow.Flow

internal sealed interface SpeechResult {
  data class Partial(val text: String) : SpeechResult
  data class Final(val text: String) : SpeechResult
  data object Completed : SpeechResult
}

internal interface SpeechClient : AutoCloseable {
  suspend fun checkStatus(): String
  suspend fun download()
  fun recognize(audio: ParcelFileDescriptor): Flow<SpeechResult>
}
