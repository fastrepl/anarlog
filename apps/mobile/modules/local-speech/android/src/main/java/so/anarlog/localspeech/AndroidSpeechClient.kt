package so.anarlog.localspeech

import android.content.Context
import android.content.Intent
import android.media.AudioFormat
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.speech.RecognitionListener
import android.speech.RecognitionSupport
import android.speech.RecognitionSupportCallback
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import androidx.annotation.RequiresApi
import java.util.Locale
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext

@RequiresApi(33)
internal class AndroidSpeechClient(
  private val context: Context,
  private val locale: String,
) : SpeechClient {
  private val main = Handler(Looper.getMainLooper())
  private var recognizer: SpeechRecognizer? = null

  private fun getRecognizer(): SpeechRecognizer {
    check(Looper.myLooper() == Looper.getMainLooper())
    if (!SpeechRecognizer.isOnDeviceRecognitionAvailable(context)) throw SpeechFailure("MODEL_UNAVAILABLE")
    return recognizer ?: SpeechRecognizer.createOnDeviceSpeechRecognizer(context).also { recognizer = it }
  }

  private fun intent(audio: ParcelFileDescriptor? = null) = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
    putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
    putExtra(RecognizerIntent.EXTRA_LANGUAGE, locale)
    putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
    if (audio != null) {
      putExtra(RecognizerIntent.EXTRA_AUDIO_SOURCE, audio)
      putExtra(RecognizerIntent.EXTRA_AUDIO_SOURCE_CHANNEL_COUNT, 1)
      putExtra(RecognizerIntent.EXTRA_AUDIO_SOURCE_SAMPLING_RATE, 16000)
      putExtra(RecognizerIntent.EXTRA_AUDIO_SOURCE_ENCODING, AudioFormat.ENCODING_PCM_16BIT)
      putExtra(RecognizerIntent.EXTRA_SEGMENTED_SESSION, RecognizerIntent.EXTRA_AUDIO_SOURCE)
    }
  }

  override suspend fun checkStatus(): String = withContext(Dispatchers.Main.immediate) {
    if (!SpeechRecognizer.isOnDeviceRecognitionAvailable(context)) return@withContext "unavailable"
    val pipe = ParcelFileDescriptor.createPipe()
    try {
      suspendCancellableCoroutine { continuation ->
        getRecognizer().checkRecognitionSupport(intent(pipe[0]), context.mainExecutor, object : RecognitionSupportCallback {
          override fun onSupportResult(support: RecognitionSupport) {
            fun List<String>.containsLocale() = any {
              it.equals(locale, ignoreCase = true) || it.equals(Locale.forLanguageTag(locale).language, ignoreCase = true)
            }
            val status = when {
              support.installedOnDeviceLanguages.containsLocale() -> "available"
              support.pendingOnDeviceLanguages.containsLocale() -> "downloading"
              support.supportedOnDeviceLanguages.containsLocale() -> "downloadable"
              else -> "unavailable"
            }
            if (continuation.isActive) continuation.resume(status)
          }
          override fun onError(error: Int) {
            if (continuation.isActive) continuation.resumeWithException(SpeechFailure("NATIVE_SUPPORT_$error"))
          }
        })
      }
    } finally { pipe.forEach { it.close() } }
  }

  override suspend fun download() {
    withContext(Dispatchers.Main.immediate) { getRecognizer().triggerModelDownload(intent()) }
    while (true) {
      delay(1500)
      when (checkStatus()) {
        "available" -> return
        "unavailable" -> throw SpeechFailure("MODEL_UNAVAILABLE")
      }
    }
  }

  override fun recognize(audio: ParcelFileDescriptor) = callbackFlow<SpeechResult> {
    withContext(Dispatchers.Main.immediate) {
      val recognizer = getRecognizer()
      recognizer.setRecognitionListener(object : RecognitionListener {
        private fun publish(result: SpeechResult) {
          if (trySend(result).isFailure) close(SpeechFailure("STREAM_FAILED"))
        }
        private fun text(results: Bundle?) = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull().orEmpty()
        override fun onPartialResults(results: Bundle?) {
          text(results).takeIf { it.isNotBlank() }?.let { publish(SpeechResult.Partial(it)) }
        }
        override fun onSegmentResults(results: Bundle) {
          text(results).takeIf { it.isNotBlank() }?.let { publish(SpeechResult.Final(it)) }
        }
        override fun onEndOfSegmentedSession() {
          publish(SpeechResult.Completed)
          close()
        }
        override fun onResults(results: Bundle?) {
          text(results).takeIf { it.isNotBlank() }?.let { publish(SpeechResult.Final(it)) }
          // A provider ignoring segmented input must not mark a partial recording complete.
          close(SpeechFailure("NATIVE_SEGMENTED_REQUIRED"))
        }
        override fun onError(error: Int) { close(SpeechFailure("NATIVE_$error")) }
        override fun onReadyForSpeech(params: Bundle?) {}
        override fun onBeginningOfSpeech() {}
        override fun onRmsChanged(rmsdB: Float) {}
        override fun onBufferReceived(buffer: ByteArray?) {}
        override fun onEndOfSpeech() {}
        override fun onEvent(eventType: Int, params: Bundle?) {}
      })
      recognizer.startListening(intent(audio))
    }
    awaitClose { this@AndroidSpeechClient.close() }
  }

  override fun close() {
    fun release() {
      recognizer?.destroy()
      recognizer = null
    }
    if (Looper.myLooper() == Looper.getMainLooper()) release() else main.post { release() }
  }
}
