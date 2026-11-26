package ai.ara.app

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.speech.tts.TextToSpeech
import java.util.Locale

/**
 * Voice in/out via Android's own on-device speech APIs — never a third-party
 * or cloud speech service. This is a deliberate product decision, not just
 * the cheapest option: the project owner asked explicitly that nothing in
 * Ara depend on a paid external service, and `SpeechRecognizer`/
 * `TextToSpeech` already ship with the OS on virtually every Android
 * phone, at no cost to the user or to Ara. Recognition quality and whether
 * it actually runs offline depend on what the device itself provides — the
 * same honest limitation already noted for a competitor taking the same
 * approach in `docs/COMPETITIVE_ANALYSIS.md` ("voice input quality and
 * offline-ness depend on what Android ships").
 */
class VoiceInput(private val context: Context) {
    private var recognizer: SpeechRecognizer? = null

    fun isAvailable(): Boolean = SpeechRecognizer.isRecognitionAvailable(context)

    /** Starts listening once. `onFinal` is called at most once per [start]
     *  call (the user reviews/edits the recognized text before sending —
     *  same rule as every other prefilled input in this app, never auto-sent). */
    fun start(
        languageTag: String,
        onPartial: (String) -> Unit,
        onFinal: (String) -> Unit,
        onError: (VoiceError) -> Unit,
    ) {
        stop()
        if (!isAvailable()) {
            onError(VoiceError.NotAvailable)
            return
        }
        val r = SpeechRecognizer.createSpeechRecognizer(context)
        recognizer = r
        r.setRecognitionListener(
            object : RecognitionListener {
                override fun onResults(results: Bundle) {
                    val text = results.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                        ?.firstOrNull().orEmpty()
                    onFinal(text)
                }
                override fun onPartialResults(results: Bundle) {
                    val text = results.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                        ?.firstOrNull().orEmpty()
                    if (text.isNotEmpty()) onPartial(text)
                }
                override fun onError(error: Int) {
                    onError(
                        when (error) {
                            SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT ->
                                VoiceError.NoSpeech
                            SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> VoiceError.NoPermission
                            else -> VoiceError.Other
                        },
                    )
                }
                override fun onReadyForSpeech(params: Bundle?) {}
                override fun onBeginningOfSpeech() {}
                override fun onRmsChanged(rmsdB: Float) {}
                override fun onBufferReceived(buffer: ByteArray?) {}
                override fun onEndOfSpeech() {}
                override fun onEvent(eventType: Int, params: Bundle?) {}
            },
        )
        r.startListening(
            Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
                putExtra(RecognizerIntent.EXTRA_LANGUAGE, languageTag)
                putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            },
        )
    }

    fun stop() {
        recognizer?.let {
            runCatching { it.stopListening() }
            it.destroy()
        }
        recognizer = null
    }
}

enum class VoiceError { NotAvailable, NoPermission, NoSpeech, Other }

/** Fully on-device TTS engine Android already ships — no network call, no
 *  API key, no cost. `speak` is a no-op until the engine finishes
 *  initializing or on a device with no TTS engine installed at all (some
 *  stripped Android builds) — both are silently safe, never a crash. */
class VoiceOutput(context: Context) {
    private var ready = false
    private val tts = TextToSpeech(context.applicationContext) { status ->
        ready = status == TextToSpeech.SUCCESS
    }

    fun speak(text: String, languageTag: String) {
        if (!ready || text.isBlank()) return
        runCatching { tts.language = Locale.forLanguageTag(languageTag) }
        tts.speak(text, TextToSpeech.QUEUE_FLUSH, null, null)
    }

    fun stop() {
        runCatching { tts.stop() }
    }

    fun shutdown() {
        runCatching { tts.shutdown() }
    }
}

private const val VOICE_PREFS = "ara_ui_prefs"
private const val KEY_SPEAK_REPLIES = "speak_replies"

fun readSpeakRepliesPref(ctx: Context): Boolean =
    ctx.getSharedPreferences(VOICE_PREFS, Context.MODE_PRIVATE).getBoolean(KEY_SPEAK_REPLIES, false)

fun writeSpeakRepliesPref(ctx: Context, value: Boolean) {
    ctx.getSharedPreferences(VOICE_PREFS, Context.MODE_PRIVATE).edit().putBoolean(KEY_SPEAK_REPLIES, value).apply()
}
