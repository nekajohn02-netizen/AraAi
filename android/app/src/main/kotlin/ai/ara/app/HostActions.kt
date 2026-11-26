package ai.ara.app

import android.app.SearchManager
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.net.Uri
import android.provider.AlarmClock
import android.provider.CalendarContract
import android.provider.MediaStore
import android.provider.Settings
import org.json.JSONObject
import java.time.LocalDateTime
import java.time.ZoneId

/**
 * Executes tool calls the Rust core approved (`run_on_host`). Each tool opens
 * the target app with content pre-filled; the user completes the action there
 * (presses send / call / save). Unknown tools are refused: this is a fixed
 * allow-list, never a generic intent launcher.
 */
object HostActions {

    private val PACKAGES = mapOf(
        "whatsapp" to "com.whatsapp",
        "telegram" to "org.telegram.messenger",
        "youtube" to "com.google.android.youtube",
        "facebook" to "com.facebook.katana",
        "instagram" to "com.instagram.android",
        "x" to "com.twitter.android",
        "tiktok" to "com.zhiliaoapp.musically",
        "linkedin" to "com.linkedin.android",
        "play_store" to "com.android.vending",
    )

    /** Returns null on success or a short error for the chat. */
    fun run(ctx: Context, tool: String, a: JSONObject): String? = try {
        when (tool) {
            "open_app" -> openApp(ctx, a.getString("app"))
            "whatsapp_compose" -> {
                val text = a.getString("text")
                val phone = a.optStringOrNull("phone")?.filter { it.isDigit() }
                if (phone != null) {
                    val uri = Uri.parse("https://wa.me/$phone?text=" + Uri.encode(text))
                    start(ctx, Intent(Intent.ACTION_VIEW, uri).setPackage("com.whatsapp"),
                        fallback = Intent(Intent.ACTION_VIEW, uri))
                } else {
                    start(ctx, sendText(text).setPackage("com.whatsapp"))
                }
            }
            "telegram_compose" -> start(ctx, sendText(a.getString("text")).setPackage("org.telegram.messenger"))
            "sms_compose" -> {
                val phone = a.optStringOrNull("phone") ?: ""
                start(ctx, Intent(Intent.ACTION_SENDTO, Uri.parse("smsto:" + Uri.encode(phone)))
                    .putExtra("sms_body", a.getString("text")))
            }
            "email_compose" -> {
                val i = Intent(Intent.ACTION_SENDTO, Uri.parse("mailto:"))
                    .putExtra(Intent.EXTRA_SUBJECT, a.getString("subject"))
                    .putExtra(Intent.EXTRA_TEXT, a.getString("body"))
                a.optStringOrNull("to")?.let { i.putExtra(Intent.EXTRA_EMAIL, arrayOf(it)) }
                start(ctx, i)
            }
            "share_text" -> {
                val text = a.getString("text")
                val pkg = PACKAGES[a.getString("target")]
                if (pkg != null) start(ctx, sendText(text).setPackage(pkg), fallback = chooser(text))
                else start(ctx, chooser(text))
            }
            "dial_number" -> start(ctx, Intent(Intent.ACTION_DIAL, Uri.parse("tel:" + Uri.encode(a.getString("phone")))))
            "set_alarm" -> start(ctx, Intent(AlarmClock.ACTION_SET_ALARM)
                .putExtra(AlarmClock.EXTRA_HOUR, a.getInt("hour"))
                .putExtra(AlarmClock.EXTRA_MINUTES, a.getInt("minute"))
                .putExtra(AlarmClock.EXTRA_MESSAGE, a.optString("label", "ARA")))
            "set_timer" -> start(ctx, Intent(AlarmClock.ACTION_SET_TIMER)
                .putExtra(AlarmClock.EXTRA_LENGTH, a.optInt("minutes") * 60 + a.optInt("seconds"))
                .putExtra(AlarmClock.EXTRA_MESSAGE, a.optString("label", "ARA"))
                .putExtra(AlarmClock.EXTRA_SKIP_UI, false))
            "calendar_event" -> {
                val start = LocalDateTime.parse(a.getString("start"))
                    .atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
                val minutes = a.optInt("duration_minutes", 60).takeIf { it > 0 } ?: 60
                start(ctx, Intent(Intent.ACTION_INSERT, CalendarContract.Events.CONTENT_URI)
                    .putExtra(CalendarContract.Events.TITLE, a.getString("title"))
                    .putExtra(CalendarContract.EXTRA_EVENT_BEGIN_TIME, start)
                    .putExtra(CalendarContract.EXTRA_EVENT_END_TIME, start + minutes * 60_000L)
                    .putExtra(CalendarContract.Events.EVENT_LOCATION, a.optString("location")))
            }
            "open_url" -> {
                val uri = Uri.parse(a.getString("url"))
                require(uri.scheme == "https") { "https only" } // re-checked here, defence in depth
                start(ctx, Intent(Intent.ACTION_VIEW, uri))
            }
            "web_search" -> {
                val q = a.getString("query")
                start(ctx, Intent(Intent.ACTION_WEB_SEARCH).putExtra(SearchManager.QUERY, q),
                    fallback = Intent(Intent.ACTION_VIEW, Uri.parse("https://duckduckgo.com/?q=" + Uri.encode(q))))
            }
            "maps_search" -> start(ctx, Intent(Intent.ACTION_VIEW, Uri.parse("geo:0,0?q=" + Uri.encode(a.getString("query")))))
            "copy_to_clipboard" -> {
                val cm = ctx.getSystemService(ClipboardManager::class.java)
                cm.setPrimaryClip(ClipData.newPlainText("ARA", a.getString("text")))
                null
            }
            "flashlight" -> {
                val cm = ctx.getSystemService(CameraManager::class.java)
                val id = cm.cameraIdList.firstOrNull {
                    cm.getCameraCharacteristics(it).get(CameraCharacteristics.FLASH_INFO_AVAILABLE) == true
                } ?: return "no flashlight on this device"
                cm.setTorchMode(id, a.getBoolean("on"))
                null
            }
            "open_settings" -> start(ctx, Intent(when (a.getString("section")) {
                "wifi" -> Settings.ACTION_WIFI_SETTINGS
                "bluetooth" -> Settings.ACTION_BLUETOOTH_SETTINGS
                "display" -> Settings.ACTION_DISPLAY_SETTINGS
                "sound" -> Settings.ACTION_SOUND_SETTINGS
                "battery" -> Intent.ACTION_POWER_USAGE_SUMMARY
                "apps" -> Settings.ACTION_APPLICATION_SETTINGS
                "location" -> Settings.ACTION_LOCATION_SOURCE_SETTINGS
                "notifications" -> Settings.ACTION_SETTINGS
                "storage" -> Settings.ACTION_INTERNAL_STORAGE_SETTINGS
                else -> Settings.ACTION_SETTINGS
            }))
            else -> "unsupported action"
        }
    } catch (e: Exception) {
        // Prefer the real message (e.g. a malformed tool argument) over the
        // exception's class name, which told the user nothing useful.
        e.message ?: e.javaClass.simpleName
    }

    private fun sendText(text: String) =
        Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text)

    private fun chooser(text: String) = Intent.createChooser(sendText(text), null)

    private fun openApp(ctx: Context, app: String): String? {
        PACKAGES[app]?.let { pkg ->
            val launch = ctx.packageManager.getLaunchIntentForPackage(pkg) ?: return "$app is not installed"
            return start(ctx, launch)
        }
        val category = when (app) {
            "gallery" -> Intent.CATEGORY_APP_GALLERY
            "maps" -> Intent.CATEGORY_APP_MAPS
            "browser" -> Intent.CATEGORY_APP_BROWSER
            "messages" -> Intent.CATEGORY_APP_MESSAGING
            "calendar" -> Intent.CATEGORY_APP_CALENDAR
            "email" -> Intent.CATEGORY_APP_EMAIL
            "calculator" -> Intent.CATEGORY_APP_CALCULATOR
            "files" -> Intent.CATEGORY_APP_FILES
            else -> null
        }
        val intent = when {
            category != null -> Intent.makeMainSelectorActivity(Intent.ACTION_MAIN, category)
            app == "camera" -> Intent(MediaStore.INTENT_ACTION_STILL_IMAGE_CAMERA)
            app == "clock" -> Intent(AlarmClock.ACTION_SHOW_ALARMS)
            app == "phone" -> Intent(Intent.ACTION_DIAL)
            app == "settings" -> Intent(Settings.ACTION_SETTINGS)
            else -> return "unknown app"
        }
        return start(ctx, intent)
    }

    private fun start(ctx: Context, intent: Intent, fallback: Intent? = null): String? {
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        return try {
            ctx.startActivity(intent); null
        } catch (e: ActivityNotFoundException) {
            if (fallback == null) "no app can handle this"
            else start(ctx, fallback, null)
        }
    }
}
