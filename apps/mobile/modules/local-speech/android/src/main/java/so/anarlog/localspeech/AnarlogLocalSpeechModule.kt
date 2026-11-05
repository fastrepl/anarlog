package so.anarlog.localspeech

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import expo.modules.kotlin.Promise
import expo.modules.kotlin.functions.Queues
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class AnarlogLocalSpeechModule : Module() {
  private val scopes = listOf("mlkit-advanced", "mlkit-basic", "android-on-device")
    .associateWith { CoroutineScope(SupervisorJob() + Dispatchers.IO) }
  @Volatile private var foreground = true
  private val sessions = ConcurrentHashMap<String, String>()
  // Keep SDK classes uninitialized on older Android versions despite the manifest override.
  private val engines = scopes.mapValues { (model, scope) -> lazy {
    OnDeviceSpeechEngine(scope, { appContext.currentActivity }, {}, { event -> sendEvent("onSpeechResult", event) }, { locale ->
      when (model) {
        "android-on-device" -> {
          if (Build.VERSION.SDK_INT < 33) throw SpeechFailure("UNSUPPORTED_DEVICE")
          AndroidSpeechClient(appContext.reactContext ?: throw SpeechFailure("INTERRUPTED"), locale)
        }
        else -> MlKitSpeechClient(locale, model == "mlkit-basic")
      }
    }, keepAwake = false)
  } }

  override fun definition() = ModuleDefinition {
    Name("AnarlogLocalSpeech")
    Events("onSpeechResult")

    AsyncFunction("isIgnoringBatteryOptimizations") {
      val context = requireNotNull(appContext.reactContext)
      context.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(context.packageName)
    }
    AsyncFunction("requestIgnoreBatteryOptimizations") {
      val activity = requireNotNull(appContext.currentActivity)
      val power = activity.getSystemService(PowerManager::class.java)
      if (!power.isIgnoringBatteryOptimizations(activity.packageName)) {
        try {
          activity.startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
            Uri.parse("package:${activity.packageName}")))
        } catch (_: ActivityNotFoundException) {
          activity.startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
      }
    }.runOnQueue(Queues.MAIN)

    AsyncFunction("checkStatus") { locale: String, model: String, promise: Promise ->
      if (!supported(model)) promise.resolve("unavailable")
      else run(promise, model) { engines.getValue(model).value.checkStatus(locale) }
    }
    AsyncFunction("downloadModel") { locale: String, model: String, promise: Promise ->
      run(promise, model) { requireForeground(); engines.getValue(model).value.downloadModel(locale); null }
    }
    AsyncFunction("transcribeFile") { uri: String, locale: String, model: String, promise: Promise ->
      run(promise, model) { engines.getValue(model).value.transcribeFile(uri, locale) }
    }
    AsyncFunction("startLive") { id: String, locale: String, model: String, promise: Promise ->
      if (sessions.putIfAbsent(id, model) != null) {
        promise.reject("ERR_MLKIT_BUSY", "Recognition already started.", null)
      } else run(promise, model) {
        Log.i("AnarlogLocalSpeech", "Starting $model")
        val start = synchronized(sessions) {
          if (sessions[id] != model) throw SpeechFailure("INTERRUPTED")
          engines.getValue(model).value.startLive(id, locale)
        }
        try { start.await(); null } catch (error: Exception) {
          sessions.remove(id, model)
          throw error
        }
      }
    }
    Function("sendAudio") { id: String, bytes: ByteArray ->
      sessions[id]?.let { engines.getValue(it).takeIf { engine -> engine.isInitialized() }?.value?.sendAudio(id, bytes) } ?: false
    }
    AsyncFunction("finishLive") { id: String, promise: Promise ->
      val model = sessions[id]
      if (model == null) promise.resolve(false)
      else run(promise, model) {
        try { engines.getValue(model).value.finishLive(id) } finally { sessions.remove(id, model) }
      }
    }
    Function("cancelLive") { id: String ->
      synchronized(sessions) {
        sessions.remove(id)?.let { model ->
          engines.getValue(model).takeIf { it.isInitialized() }?.value?.cancelLive(id)
        }
      }
    }

    OnActivityEntersForeground {
      foreground = true
      Log.i("AnarlogLocalSpeech", "App foregrounded; activeSessions=${sessions.size}")
    }
    OnActivityEntersBackground {
      foreground = false
      Log.i("AnarlogLocalSpeech", "App backgrounded; activeSessions=${sessions.size}")
    }
    OnDestroy {
      engines.values.forEach { if (it.isInitialized()) it.value.cancelAll() }
      scopes.values.forEach { it.cancel() }
      sessions.clear()
    }
  }

  private fun supported(model: String) = model in scopes && Build.VERSION.SDK_INT >= if (model == "android-on-device") 33 else 31

  private fun requireForeground() {
    if (!foreground || appContext.currentActivity == null) throw SpeechFailure("FOREGROUND_REQUIRED")
  }

  private fun run(promise: Promise, model: String, operation: suspend () -> Any?) {
    if (!supported(model)) {
      promise.reject("ERR_MLKIT_UNSUPPORTED_DEVICE", "Local speech recognition is unavailable on this device.", null)
      return
    }
    scopes.getValue(model).launch(start = CoroutineStart.UNDISPATCHED) {
      try {
        promise.resolve(withContext(Dispatchers.IO) {
          operation()
        })
      } catch (error: SpeechFailure) {
        promise.reject("ERR_MLKIT_${error.reason}", "Local speech recognition could not finish.", null)
      } catch (error: CancellationException) {
        promise.reject("ERR_MLKIT_INTERRUPTED", "Local speech recognition was interrupted.", null)
      } catch (error: Exception) {
        // Provider exceptions may contain transcript data; expose only a stable code to JS.
        promise.reject("ERR_MLKIT_FAILED", "Google on-device speech recognition failed.", null)
      }
    }
  }
}

internal class SpeechFailure(val reason: String) : Exception()
