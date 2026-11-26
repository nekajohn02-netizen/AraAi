package ai.ara.app

import android.annotation.SuppressLint
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.OpenableColumns
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uniffi.ara_ffi.AraCore
import uniffi.ara_ffi.CancelHandle
import uniffi.ara_ffi.ProgressListener
import uniffi.ara_ffi.TextListener

/**
 * Kotlin side of the bridge. Owns the single [AraCore] instance, gathers
 * Android-only facts (thermal state, device model) and runs every core call
 * off the main thread. All decisions (model fit, tool policy, context) are
 * made in Rust; this class only adapts types.
 */
@SuppressLint("StaticFieldLeak") // the singleton only ever holds applicationContext (below), never an Activity
class CoreBridge private constructor(private val context: Context) {

    val core: AraCore = AraCore(context.filesDir.resolve("ara").path)

    companion object {
        @Volatile private var instance: CoreBridge? = null
        fun get(context: Context): CoreBridge =
            instance ?: synchronized(this) {
                instance ?: CoreBridge(context.applicationContext).also { instance = it }
            }

        /**
         * Same singleton as [get], but opens the on-disk store (SQLite index,
         * model/conversation/mission directories) off the main thread. The
         * first frame must render before this finishes — callers show a
         * loading state (the splash screen) until it completes instead of
         * blocking [android.app.Activity.onCreate].
         */
        suspend fun getAsync(context: Context): CoreBridge =
            instance ?: withContext(Dispatchers.IO) { get(context) }
    }

    fun hostHintsJson(): String {
        val hints = JSONObject()
            .put("device_model", Build.MODEL)
            .put("os_version", Build.VERSION.RELEASE)
        thermalState()?.let { hints.put("thermal", it) }
        return hints.toString()
    }

    private fun thermalState(): String? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return null
        val pm = context.getSystemService(PowerManager::class.java) ?: return null
        return when (pm.currentThermalStatus) {
            PowerManager.THERMAL_STATUS_NONE -> "none"
            PowerManager.THERMAL_STATUS_LIGHT -> "light"
            PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
            PowerManager.THERMAL_STATUS_SEVERE -> "severe"
            else -> "critical"
        }
    }

    suspend fun <T> io(block: AraCore.() -> T): T = withContext(Dispatchers.IO) { core.block() }

    suspend fun models(mode: String, language: String, persona: String): JSONObject =
        io { JSONObject(models(hostHintsJson(), mode, language, persona)) }

    suspend fun personas(): List<JSONObject> = io { JSONArray(personas()) }.objects()

    suspend fun download(id: String, cancel: CancelHandle, onProgress: (Long, Long) -> Unit) = io {
        downloadModel(id, object : ProgressListener {
            override fun onProgress(done: ULong, total: ULong) = onProgress(done.toLong(), total.toLong())
        }, cancel)
    }

    suspend fun send(convId: String, text: String, cancel: CancelHandle, onText: (String) -> Unit): JSONObject =
        io {
            JSONObject(sendMessage(convId, text, object : TextListener {
                override fun onText(text: String) = onText(text)
            }, cancel))
        }

    suspend fun conversations(): JSONArray = io { JSONArray(conversations()) }
    suspend fun conversation(id: String): JSONObject = io { JSONObject(conversation(id)) }
}

/**
 * Call at the top of any `catch (e: Exception)` block around a suspend call.
 * `CancellationException` is a subtype of `Exception`, so a bare catch would
 * otherwise swallow it — breaking structured concurrency: cancelling a
 * screen's `rememberCoroutineScope()` (e.g. leaving the screen mid-request)
 * would then surface as a user-visible "error" instead of silently stopping,
 * and the coroutine would never actually finish cancelling.
 */
fun Throwable.rethrowIfCancellation(): Throwable {
    if (this is CancellationException) throw this
    return this
}

fun JSONArray.objects(): List<JSONObject> = (0 until length()).map { getJSONObject(it) }

/** Display name + byte size of a picked document, read without opening it. */
fun nameAndSize(ctx: Context, uri: Uri): Pair<String, Long> {
    ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)
        ?.use { c ->
            if (c.moveToFirst()) {
                val name = c.getString(0) ?: "file.txt"
                val size = if (c.isNull(1)) 0L else c.getLong(1)
                return name to size
            }
        }
    return (uri.lastPathSegment ?: "file.txt") to 0L
}

fun JSONArray.strings(): List<String> = (0 until length()).map { getString(it) }

fun JSONObject.optStringOrNull(key: String): String? =
    if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }
