package ai.ara.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
 * API Hub: bring-your-own-key cloud models (Kimi, DeepSeek, OpenAI …).
 * Nothing is sent before the user accepts the consent dialog for that
 * provider; the Rust core enforces it again.
 */
@Composable
fun ApiHubSection(bridge: CoreBridge, onChanged: () -> Unit) {
    val ctx = LocalContext.current
    val vault = remember { KeyVault(ctx) }
    val scope = rememberCoroutineScope()
    var presets by remember { mutableStateOf(emptyList<JSONObject>()) }
    var providers by remember { mutableStateOf(emptyList<JSONObject>()) }
    var reload by remember { mutableIntStateOf(0) }
    var preset by remember { mutableStateOf<JSONObject?>(null) }
    var baseUrl by remember { mutableStateOf("") }
    var apiKey by remember { mutableStateOf("") }
    var model by remember { mutableStateOf("") }
    var available by remember { mutableStateOf(emptyList<String>()) }
    var shield by remember { mutableStateOf(true) }
    var status by remember { mutableStateOf<String?>(null) }
    var consentFor by remember { mutableStateOf<JSONObject?>(null) }

    LaunchedEffect(reload) {
        presets = bridge.io { JSONArray(cloudPresets()) }.objects()
        providers = bridge.io { JSONArray(providers()) }.objects()
    }

    fun activate(p: JSONObject) = scope.launch {
        val key = vault.get(p.getString("id"))
        if (key == null) { status = ctx.getString(R.string.key_missing); return@launch }
        status = try { bridge.io { useCloudModel(p.getString("id"), key) }; onChanged(); null }
        catch (e: Exception) { e.message }
    }

    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.api_hub_title), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.api_hub_note), style = MaterialTheme.typography.bodySmall)

            providers.forEach { p ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("☁ ${p.getString("name")} · ${p.getString("model")}")
                        Text(if (p.optBoolean("shield", true)) stringResource(R.string.shield_on)
                            else stringResource(R.string.shield_off), style = MaterialTheme.typography.bodySmall)
                    }
                    if (p.optBoolean("consent")) Button({ activate(p) }) { Text(stringResource(R.string.use_model)) }
                    else OutlinedButton({ consentFor = p }) { Text(stringResource(R.string.allow_sending)) }
                    TextButton({
                        scope.launch {
                            bridge.io { deleteProvider(p.getString("id")) }
                            vault.remove(p.getString("id")); reload++; onChanged()
                        }
                    }) { Text(stringResource(R.string.delete)) }
                }
            }

            Text(stringResource(R.string.add_provider), style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                presets.forEach { pr ->
                    FilterChip(selected = preset?.getString("id") == pr.getString("id"), onClick = {
                        preset = pr; baseUrl = pr.getString("base_url"); available = emptyList(); model = ""
                    }, label = { Text(pr.getString("name")) })
                }
            }
            preset?.let { pr ->
                if (pr.optString("note").isNotEmpty()) Text(pr.getString("note"), style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(baseUrl, { baseUrl = it }, Modifier.fillMaxWidth(), label = { Text("Base URL") }, singleLine = true)
                OutlinedTextField(apiKey, { apiKey = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.api_key)) },
                    singleLine = true, visualTransformation = PasswordVisualTransformation())
                OutlinedButton(enabled = apiKey.isNotBlank(), onClick = {
                    scope.launch {
                        status = try {
                            available = bridge.io { cloudModels(baseUrl, apiKey) }
                            ctx.getString(R.string.connection_ok, available.size)
                        } catch (e: Exception) { e.message }
                    }
                }) { Text(stringResource(R.string.test_connection)) }
                if (available.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    available.take(30).forEach { m -> FilterChip(model == m, { model = m }, { Text(m) }) }
                }
                OutlinedTextField(model, { model = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.model_id)) }, singleLine = true)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.privacy_shield), Modifier.weight(1f))
                    Switch(shield, { shield = it })
                }
                Button(enabled = apiKey.isNotBlank() && model.isNotBlank(), onClick = {
                    val id = pr.getString("id") + "-" + model.filter { it.isLetterOrDigit() }.take(24).lowercase()
                    val json = JSONObject().put("id", id).put("name", pr.getString("name"))
                        .put("base_url", baseUrl).put("model", model).put("shield", shield).toString()
                    scope.launch {
                        status = try {
                            bridge.io { saveProvider(json) }
                            vault.put(id, apiKey)
                            apiKey = ""; preset = null; reload++
                            null
                        } catch (e: Exception) { e.message }
                    }
                }) { Text(stringResource(R.string.save)) }
            }
            status?.let { Text(it, color = MaterialTheme.colorScheme.secondary) }
        }
    }

    consentFor?.let { p ->
        AlertDialog(
            onDismissRequest = { consentFor = null },
            title = { Text(stringResource(R.string.consent_title, p.getString("name"))) },
            text = { Text(stringResource(R.string.consent_body)) },
            confirmButton = {
                Button({
                    scope.launch { bridge.io { setCloudConsent(p.getString("id"), true) }; consentFor = null; reload++ }
                }) { Text(stringResource(R.string.agree)) }
            },
            dismissButton = { TextButton({ consentFor = null }) { Text(stringResource(R.string.decline)) } },
        )
    }
}
