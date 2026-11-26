package ai.ara.app

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/**
 * Smart home (ADR-020): connect the user's own Home Assistant and toggle
 * the devices it already controls. Ara never speaks a device vendor's
 * protocol directly — this talks to the hub the user already runs.
 *
 * The access token is stored the same way an API Hub provider key is: in
 * [KeyVault] (Android Keystore-backed), never sent to the Rust core's
 * on-disk settings — only the base URL is persisted there
 * (`AraCore::smarthomeBaseUrl`). Each device toggle here is the user's own
 * tap, which **is** the confirmation (same UX precedent as a light switch);
 * this is deliberately separate from the chat tool `smarthome_control_device`,
 * which the model can request and which always stops for a *separate*
 * confirmation card first (ADR-006/ADR-020) — this screen never bypasses that.
 */
@Composable
fun SmartHomeSection(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val vault = remember { KeyVault(ctx) }
    val scope = rememberCoroutineScope()
    var connected by remember { mutableStateOf(false) }
    var baseUrl by remember { mutableStateOf("") }
    var token by remember { mutableStateOf("") }
    var devices by remember { mutableStateOf(emptyList<JSONObject>()) }
    var status by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var reload by remember { mutableIntStateOf(0) }
    // Captured here, not via `ctx.getString(...)` inside the lambda below —
    // see the matching note in ApiHubSection.kt (NonObservableLocale-style
    // issue for context-fetched strings).
    val connectedCountTemplate = stringResource(R.string.smarthome_connected_count)

    LaunchedEffect(Unit) {
        connected = bridge.io { smarthomeConnected() }
        baseUrl = bridge.io { smarthomeBaseUrl() }.orEmpty()
        if (baseUrl.isNotBlank()) vault.get("home_assistant")?.let { token = it }
        // The Rust session is in-memory only (ADR-020: the token is never
        // persisted), so a fresh app process always starts "disconnected"
        // even though the base URL and the Keystore-backed token both
        // survived the restart. Silently reconnect with what's already
        // saved instead of making the user reconnect by hand every launch
        // — this performs no network call itself (`smarthomeConnect` only
        // stores the session; `smarthomeDevices`, loaded next, is what
        // actually reaches the network and will surface a real error here).
        if (!connected && baseUrl.isNotBlank() && token.isNotBlank()) {
            status = try {
                bridge.io { smarthomeConnect(baseUrl, token) }
                connected = true
                null
            } catch (e: Exception) {
                e.rethrowIfCancellation().message
            }
        }
    }
    LaunchedEffect(reload, connected) {
        if (!connected) return@LaunchedEffect
        status = try {
            devices = JSONArray(bridge.io { smarthomeDevices() }).objects()
            null
        } catch (e: Exception) {
            e.rethrowIfCancellation().message
        }
    }

    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.smarthome_title), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.smarthome_note), style = MaterialTheme.typography.bodySmall)

            if (!connected) {
                OutlinedTextField(
                    baseUrl, { baseUrl = it }, Modifier.fillMaxWidth(),
                    label = { Text(stringResource(R.string.smarthome_url)) },
                    placeholder = { Text("http://homeassistant.local:8123") },
                    singleLine = true,
                )
                OutlinedTextField(
                    token, { token = it }, Modifier.fillMaxWidth(),
                    label = { Text(stringResource(R.string.smarthome_token)) },
                    singleLine = true, visualTransformation = PasswordVisualTransformation(),
                )
                Button(enabled = !busy && baseUrl.isNotBlank() && token.isNotBlank(), onClick = {
                    scope.launch {
                        busy = true
                        status = try {
                            val count = bridge.io { smarthomeTestConnection(baseUrl, token) }
                            bridge.io { smarthomeConnect(baseUrl, token) }
                            vault.put("home_assistant", token)
                            connected = true
                            reload++
                            String.format(connectedCountTemplate, count.toInt())
                        } catch (e: Exception) {
                            e.rethrowIfCancellation().message
                        }
                        busy = false
                    }
                }) { Text(stringResource(R.string.smarthome_connect)) }
            } else {
                Text(stringResource(R.string.smarthome_connected_to, baseUrl), style = MaterialTheme.typography.bodySmall)
                devices.forEach { d ->
                    val entityId = d.getString("entity_id")
                    val domain = entityId.substringBefore(".")
                    val state = d.getString("state")
                    val name = d.optStringOrNull("friendly_name") ?: entityId
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(name)
                            Text(state, style = MaterialTheme.typography.bodySmall)
                        }
                        fun act(service: String) = scope.launch {
                            status = try {
                                bridge.io { smarthomeCallService(domain, service, entityId) }
                                reload++
                                null
                            } catch (e: Exception) {
                                e.rethrowIfCancellation().message
                            }
                        }
                        when (domain) {
                            "light", "switch", "fan", "input_boolean" ->
                                Switch(state == "on", { act(if (it) "turn_on" else "turn_off") })
                            "lock" -> Switch(state == "locked", { act(if (it) "lock" else "unlock") })
                            else -> {}
                        }
                    }
                }
                OutlinedButton({
                    scope.launch {
                        bridge.io { smarthomeDisconnect() }
                        vault.remove("home_assistant")
                        connected = false
                        devices = emptyList()
                        token = ""
                    }
                }) { Text(stringResource(R.string.smarthome_disconnect)) }
            }

            AnimatedVisibility(
                status != null, enter = fadeIn(tween(motionMillis())), exit = fadeOut(tween(motionMillis())),
            ) {
                Text(status.orEmpty(), color = MaterialTheme.colorScheme.error, modifier = Modifier.liveRegionAnnounce())
            }
        }
    }
}
