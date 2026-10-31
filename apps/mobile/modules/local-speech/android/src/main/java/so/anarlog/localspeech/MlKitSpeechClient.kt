package so.anarlog.localspeech

import android.os.ParcelFileDescriptor
import android.util.Log
import com.google.mlkit.genai.common.GenAiException
import com.google.mlkit.genai.common.DownloadStatus
import com.google.mlkit.genai.common.FeatureStatus
import com.google.mlkit.genai.common.audio.AudioSource
import com.google.mlkit.genai.speechrecognition.SpeechRecognition
import com.google.mlkit.genai.speechrecognition.SpeechRecognizer
import com.google.mlkit.genai.speechrecognition.SpeechRecognizerOptions
import com.google.mlkit.genai.speechrecognition.SpeechRecognizerResponse
import com.google.mlkit.genai.speechrecognition.speechRecognizerOptions
import com.google.mlkit.genai.speechrecognition.speechRecognizerRequest
import java.util.Locale
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.catch

internal class MlKitSpeechClient(private val recognizer: SpeechRecognizer) : SpeechClient {
  constructor(locale: String, basic: Boolean) : this(SpeechRecognition.getClient(speechRecognizerOptions {
    this.locale = Locale.forLanguageTag(locale)
    preferredMode = if (basic) SpeechRecognizerOptions.Mode.MODE_BASIC else SpeechRecognizerOptions.Mode.MODE_ADVANCED
  }))

  override suspend fun checkStatus() = when (recognizer.checkStatus()) {
    FeatureStatus.AVAILABLE -> "available"
    FeatureStatus.DOWNLOADABLE -> "downloadable"
    FeatureStatus.DOWNLOADING -> "downloading"
    else -> "unavailable"
  }

  override suspend fun download() {
    recognizer.download().collect { if (it is DownloadStatus.DownloadFailed) throw it.e }
  }

  override fun recognize(audio: ParcelFileDescriptor) = recognizer.startRecognition(
    speechRecognizerRequest { audioSource = AudioSource.fromPfd(audio) }
  ).map { response ->
    when (response) {
      is SpeechRecognizerResponse.PartialTextResponse -> SpeechResult.Partial(response.text)
      is SpeechRecognizerResponse.FinalTextResponse -> SpeechResult.Final(response.text)
      is SpeechRecognizerResponse.CompletedResponse -> SpeechResult.Completed
      is SpeechRecognizerResponse.ErrorResponse -> throw response.e
    }
  }.catch { error ->
    if (error is GenAiException) {
      Log.w("AnarlogLocalSpeech", "ML Kit recognition failed: sdkCode=${error.errorCode}")
      if (error.errorCode == GenAiException.ErrorCode.BACKGROUND_USE_BLOCKED) {
        throw SpeechFailure("BACKGROUND_USE_BLOCKED")
      }
    }
    throw error
  }

  override fun close() = recognizer.close()
}
