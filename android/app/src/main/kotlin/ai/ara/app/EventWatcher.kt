package ai.ara.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * Listens for OS-level signals (battery low, charging state, connectivity)
 * and, for each one, asks the Rust core whether any user-configured rule
 * should turn it into a mission (`AraCore.fireEvent`). Every listener here
 * is push-based (the OS calls us) — there is no polling loop, so this
 * costs nothing beyond what the OS already tracks internally.
 */
object EventWatcher {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var registered = false

    @Synchronized
    fun register(context: Context) {
        if (registered) return
        registered = true

        val receiver = object : BroadcastReceiver() {
            override fun onReceive(ctx: Context, intent: Intent) {
                val kind = when (intent.action) {
                    Intent.ACTION_BATTERY_LOW -> "battery_low"
                    Intent.ACTION_POWER_CONNECTED -> "power_connected"
                    Intent.ACTION_POWER_DISCONNECTED -> "power_disconnected"
                    else -> return
                }
                fire(ctx, kind)
            }
        }
        // On Android 13+ (targetSdk here is 36), the plain 2-arg
        // `registerReceiver` throws `SecurityException` at the call site
        // unless RECEIVER_EXPORTED/RECEIVER_NOT_EXPORTED is given — a real
        // crash this app would hit on launch on any current-generation
        // phone, found while auditing this file for other startup crashes
        // 2026-11-26. `ContextCompat.registerReceiver` is the
        // version-safe way to pass that flag (plain `Context.
        // registerReceiver(receiver, filter, flags)` needs API 33+ itself).
        // NOT_EXPORTED: only this app's own process needs these broadcasts.
        ContextCompat.registerReceiver(
            context,
            receiver,
            IntentFilter().apply {
                addAction(Intent.ACTION_BATTERY_LOW)
                addAction(Intent.ACTION_POWER_CONNECTED)
                addAction(Intent.ACTION_POWER_DISCONNECTED)
            },
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )

        context.getSystemService(ConnectivityManager::class.java)?.registerDefaultNetworkCallback(
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) = fire(context, "network_available")
                override fun onLost(network: Network) = fire(context, "network_lost")
            },
        )
    }

    private fun fire(context: Context, kind: String) {
        scope.launch {
            val created = runCatching { CoreBridge.get(context).io { fireEvent(kind) } }.getOrNull().orEmpty()
            created.forEach { id -> MissionWorker.enqueueNext(context, id) }
        }
    }
}
