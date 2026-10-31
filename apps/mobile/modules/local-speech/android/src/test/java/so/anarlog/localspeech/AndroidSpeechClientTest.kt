package so.anarlog.localspeech

import android.os.Bundle
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.toList
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.LooperMode
import org.robolectric.shadows.ShadowSpeechRecognizer

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@LooperMode(LooperMode.Mode.LEGACY)
class AndroidSpeechClientTest {
  @Test fun segmentedRecognitionPreservesFinalTextButRejectsAnUtteranceOnlyProvider() = runBlocking {
    for (segmented in listOf(true, false)) {
      ShadowSpeechRecognizer.reset()
      ShadowSpeechRecognizer.setIsOnDeviceRecognitionAvailable(true)
      val client = AndroidSpeechClient(RuntimeEnvironment.getApplication(), "en-US")
      val pipe = ParcelFileDescriptor.createPipe()
      try {
        val result = async { runCatching { client.recognize(pipe[0]).toList() } }
        withTimeout(3000) {
          while (ShadowSpeechRecognizer.getLatestSpeechRecognizer() == null) { yield(); delay(10) }
        }
        val shadow = shadowOf(ShadowSpeechRecognizer.getLatestSpeechRecognizer())
        withTimeout(3000) { while (shadow.lastRecognizerIntent == null) { yield(); delay(10) } }
        val intent = shadow.lastRecognizerIntent
        assertEquals(RecognizerIntent.EXTRA_AUDIO_SOURCE, intent.getStringExtra(RecognizerIntent.EXTRA_SEGMENTED_SESSION))
        assertNotNull(intent.getParcelableExtra(RecognizerIntent.EXTRA_AUDIO_SOURCE, ParcelFileDescriptor::class.java))
        val text = Bundle().apply { putStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION, arrayListOf("Keep this sentence")) }
        if (segmented) {
          shadow.triggerOnSegmentResults(text)
          shadow.triggerOnEndOfSegmentedSession()
        } else shadow.triggerOnResults(text)
        val outcome = withTimeout(3000) { result.await() }
        if (segmented) assertEquals(listOf(SpeechResult.Final("Keep this sentence"), SpeechResult.Completed), outcome.getOrThrow())
        else assertEquals("NATIVE_SEGMENTED_REQUIRED", (outcome.exceptionOrNull() as SpeechFailure).reason)
        client.close()
        shadowOf(Looper.getMainLooper()).idle()
        assertTrue(shadow.isDestroyed)
      } finally { client.close(); pipe.forEach { it.close() } }
    }
  }
}
