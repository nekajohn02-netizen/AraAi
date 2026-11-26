package ai.ara.app

import android.app.Application
import android.util.Log

/**
 * Registers the Event Watcher's OS listeners for the whole process
 * lifetime, not just while an Activity happens to be alive — the closest
 * thing to a "background daemon" Android allows without a foreground
 * service (which would mean a permanent notification and more battery
 * use, working against this project's own resource-conservation goals).
 *
 * Honest limitation: this covers the process while it's alive, including
 * while the app is backgrounded — it does **not** survive the OS killing
 * the process outright. That's not a bug to fix; it's how every app on
 * Android works without becoming the kind of battery-draining background
 * service Android 8+ was specifically designed to stop.
 */
class AraApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        // Registering OS listeners (connectivity callback, a broadcast
        // receiver) can throw on some OEM/Android-version combinations —
        // a real possibility flagged 2026-11-25/26 while chasing an
        // unrelated crash-on-launch report. Event Watcher is an optional
        // convenience; it must never be able to take the whole app down
        // before MainActivity even gets a chance to run.
        runCatching { EventWatcher.register(this) }
            .onFailure { Log.e("Ara", "EventWatcher registration failed", it) }
    }
}
