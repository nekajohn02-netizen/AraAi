package ai.ara.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
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
        context.registerReceiver(
            receiver,
            IntentFilter().apply {
                addAction(Intent.ACTION_BATTERY_LOW)
                addAction(Intent.ACTION_POWER_CONNECTED)
                addAction(Intent.ACTION_POWER_DISCONNECTED)
            },
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
