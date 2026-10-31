package so.anarlog.localspeech

import android.app.Activity
import android.os.ParcelFileDescriptor
import com.google.mlkit.genai.common.audio.AudioSource
import com.google.mlkit.genai.common.FeatureStatus
import com.google.mlkit.genai.common.GenAiException
import com.google.mlkit.genai.speechrecognition.SpeechRecognizer
import com.google.mlkit.genai.speechrecognition.SpeechRecognizerRequest
import com.google.mlkit.genai.speechrecognition.SpeechRecognizerResponse
import com.google.mlkit.genai.speechrecognition.speechRecognizerRequest
import kotlin.coroutines.Continuation
import kotlin.coroutines.intrinsics.startCoroutineUninterceptedOrReturn
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.mockito.ArgumentMatchers.any
import org.mockito.Mockito.*
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements
import org.robolectric.annotation.RealObject
import org.robolectric.shadow.api.Shadow

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [31], manifest = Config.NONE, shadows = [SpeechOutputShadow::class])
class LiveSpeechLifecycleTest {
  private fun anyRequest(): SpeechRecognizerRequest {
    any(SpeechRecognizerRequest::class.java)
    return speechRecognizerRequest { audioSource = AudioSource.fromMic() }
  }

  @Test fun cancellingDuringStartupReleasesRecognizerAndAllowsAnotherStart() = runBlocking {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val recognizer = mock(SpeechRecognizer::class.java)
    var checking = CompletableDeferred<Unit>()
    var closed = CompletableDeferred<Unit>()
    doAnswer { invocation ->
      val check: suspend () -> Int = { checking.complete(Unit); awaitCancellation() }
      @Suppress("UNCHECKED_CAST")
      check.startCoroutineUninterceptedOrReturn(invocation.rawArguments.last() as Continuation<Int>)
    }.`when`(recognizer).checkStatus()
    doAnswer { closed.complete(Unit); null }.`when`(recognizer).close()
    val engine = OnDeviceSpeechEngine(scope, { null }, {}, {}, { MlKitSpeechClient(recognizer) })
    try {
      for (cancelAll in listOf(false, true)) {
        checking = CompletableDeferred()
        closed = CompletableDeferred()
        val start = engine.startLive("note", "en-US")
        withTimeout(2000) { checking.await() }
        if (cancelAll) engine.cancelAll() else engine.cancelLive("note")
        withTimeout(2000) { start.join(); closed.await() }
        assertTrue(start.isCancelled)
        assertFalse(engine.sendAudio("note", byteArrayOf(1, 2)))
      }
    } finally { scope.cancel() }
  }

  @Test fun cancellingLiveRecognitionClosesTheStreamAndReleasesTheRecognitionLock() = runBlocking {
    val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val recognizer = mock(SpeechRecognizer::class.java)
    `when`(recognizer.checkStatus()).thenReturn(FeatureStatus.AVAILABLE)
    var listening = CompletableDeferred<Unit>()
    var closed = false
    val events = java.util.concurrent.CopyOnWriteArrayList<Map<String, Any>>()
    `when`(recognizer.startRecognition(anyRequest())).thenAnswer {
      flow<SpeechRecognizerResponse> { listening.complete(Unit); awaitCancellation() }
    }
    doAnswer { closed = true; null }.`when`(recognizer).close()
    val engine = OnDeviceSpeechEngine(scope, { activity }, {}, { events.add(it) }, { MlKitSpeechClient(recognizer) })
    try {
      repeat(2) { index ->
        listening = CompletableDeferred()
        closed = false
        withTimeout(2000) { engine.startLive("note-$index", "en-US").await(); listening.await() }
        engine.cancelAll()
        withTimeout(2000) { scope.coroutineContext.job.children.toList().joinAll() }
        assertTrue(closed)
        assertFalse(engine.sendAudio("note-$index", byteArrayOf(1, 2)))
        assertEquals("error", events.last()["type"])
      }
    } finally { scope.cancel(); activity.finish() }
  }

  @Test fun sdkBackgroundRejectionIsReportedWithoutMarkingTheRecordingComplete() = runBlocking {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val recognizer = mock(SpeechRecognizer::class.java)
    val events = java.util.concurrent.CopyOnWriteArrayList<Map<String, Any>>()
    val released = CompletableDeferred<Unit>()
    `when`(recognizer.startRecognition(anyRequest())).thenReturn(flow {
      throw GenAiException(Exception("synthetic provider detail"), GenAiException.ErrorCode.BACKGROUND_USE_BLOCKED)
    })
    try {
      val live = LiveSpeech(MlKitSpeechClient(recognizer)) { events.add(it) }
      live.start(scope) { released.complete(Unit) }
      assertFalse(withTimeout(3000) { live.finish() })
      withTimeout(2000) { released.await() }
      assertEquals("error", events.last()["type"])
      assertEquals("ERR_MLKIT_BACKGROUND_USE_BLOCKED", events.last()["code"])
    } finally { scope.cancel() }
  }

  @Test fun completionRequiresAllQueuedAudioToReachThePipe() = runBlocking {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val bytes = ByteArray(6400) { it.toByte() }
    try {
      for (consumeAudio in listOf(false, true)) {
        SpeechOutputShadow.closed = CompletableDeferred()
        val recognizer = mock(SpeechRecognizer::class.java)
        val received = CompletableDeferred<ByteArray>()
        val released = CompletableDeferred<Unit>()
        val events = java.util.concurrent.CopyOnWriteArrayList<Map<String, Any>>()
        `when`(recognizer.startRecognition(anyRequest())).thenAnswer { invocation ->
          val request = invocation.getArgument<SpeechRecognizerRequest>(0)
          if (!consumeAudio) flowOf(SpeechRecognizerResponse.CompletedResponse)
          else flow {
            // Robolectric backs pipes with files, so emulate EOF by awaiting the writer's close.
            SpeechOutputShadow.closed.await()
            val audio = withContext(Dispatchers.IO) {
              // The alpha's Kotlin metadata names a getter missing from its published bytecode.
              val source = request.javaClass.declaredMethods.single {
                it.parameterCount == 0 && it.returnType == AudioSource::class.java
              }.invoke(request) as AudioSource
              ParcelFileDescriptor.AutoCloseInputStream(source.pfd).use { it.readBytes() }
            }
            received.complete(audio)
            emit(SpeechRecognizerResponse.CompletedResponse)
          }
        }
        val live = LiveSpeech(MlKitSpeechClient(recognizer)) { events.add(it) }
        assertTrue(live.send(bytes))
        live.start(scope) { released.complete(Unit) }
        val complete = withTimeout(3000) { live.finish() }
        withTimeout(2000) { released.await() }
        assertEquals(org.robolectric.shadows.ShadowLog.getLogsForTag("AnarlogLocalSpeech").joinToString { it.msg }, consumeAudio, complete)
        assertEquals(if (consumeAudio) "completed" else "error", events.last()["type"])
        if (consumeAudio) assertArrayEquals(bytes, received.await())
      }
    } finally { scope.cancel() }
  }
}


@Implements(ParcelFileDescriptor.AutoCloseOutputStream::class)
class SpeechOutputShadow {
  @RealObject private lateinit var output: ParcelFileDescriptor.AutoCloseOutputStream

  @Implementation fun close() {
    Shadow.directlyOn<Void, ParcelFileDescriptor.AutoCloseOutputStream>(output, ParcelFileDescriptor.AutoCloseOutputStream::class.java, "close")
    closed.complete(Unit)
  }

  companion object {
    var closed = CompletableDeferred<Unit>()
  }
}
