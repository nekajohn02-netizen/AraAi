package ai.ara.app

import android.text.format.Formatter
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

private val GRANTS = listOf(
    "denied" to R.string.grant_denied,
    "ask_every_time" to R.string.grant_ask,
    "always" to R.string.grant_always,
)

@Composable
fun SkillsScreen(bridge: CoreBridge) {
    val scope = rememberCoroutineScope()
    var skills by remember { mutableStateOf(emptyList<JSONObject>()) }
    var grants by remember { mutableStateOf(JSONObject()) }
    var reload by remember { mutableIntStateOf(0) }
    val arabic = Locale.getDefault().language == "ar"

    LaunchedEffect(reload) {
        skills = bridge.io { JSONArray(skills()) }.objects()
        grants = bridge.io { JSONObject(permissions()) }.optJSONObject("grants") ?: JSONObject()
    }

    LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.skills_title), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(R.string.skills_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        items(skills) { row ->
            val s = row.getJSONObject("skill")
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(s.getString(if (arabic) "name_ar" else "name_en"), style = MaterialTheme.typography.titleMedium)
                    Text(s.getString("description"), style = MaterialTheme.typography.bodySmall)
                    val perms = row.getJSONArray("permissions")
                    if (perms.length() == 0) Text(stringResource(R.string.no_permission_needed))
                    for (i in 0 until perms.length()) {
                        val p = perms.getString(i)
                        val current = grants.optString(p, "denied")
                        Text(permissionLabel(p), style = MaterialTheme.typography.labelLarge)
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            GRANTS.forEach { (g, label) ->
                                FilterChip(selected = current == g, onClick = {
                                    scope.launch { bridge.io { setPermission(p, g) }; reload++ }
                                }, label = { Text(stringResource(label)) })
                            }
                        }
                    }
                }
            }
        }
        item { Text(stringResource(R.string.external_always_confirm), style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(bottom = 24.dp)) }
    }
}

@Composable
private fun permissionLabel(p: String): String = stringResource(when (p) {
    "launch_apps" -> R.string.perm_launch_apps
    "compose_messages" -> R.string.perm_compose_messages
    "share_content" -> R.string.perm_share
    "phone_dialer" -> R.string.perm_dialer
    "alarms" -> R.string.perm_alarms
    "calendar" -> R.string.perm_calendar
    "clipboard" -> R.string.perm_clipboard
    "device_controls" -> R.string.perm_device
    "open_web" -> R.string.perm_web
    "memory" -> R.string.perm_memory
    else -> R.string.perm_files
})

@Composable
fun SettingsScreen(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var storage by remember { mutableStateOf<JSONObject?>(null) }
    var reload by remember { mutableIntStateOf(0) }
    LaunchedEffect(reload) { storage = bridge.io { JSONObject(storageInfo()) } }
    val gib = 1L shl 30

    LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item { Text(stringResource(R.string.tab_settings), style = MaterialTheme.typography.headlineSmall,
            modifier = Modifier.padding(top = 16.dp)) }
        storage?.let { st ->
            item {
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(stringResource(R.string.storage_limit), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.storage_used,
                            Formatter.formatShortFileSize(ctx, st.getLong("models_bytes"))))
                        val cap = if (st.isNull("cap_bytes")) 0L else st.getLong("cap_bytes")
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            listOf(0L, 2 * gib, 4 * gib, 8 * gib).forEach { v ->
                                FilterChip(selected = cap == v, onClick = {
                                    scope.launch { bridge.io { setStorageCap(v.toULong()) }; reload++ }
                                }, label = { Text(if (v == 0L) stringResource(R.string.no_limit) else "${v / gib} GB") })
                            }
                        }
                        Text(stringResource(R.string.context_size), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.context_note), style = MaterialTheme.typography.bodySmall)
                        val ctxTokens = st.getInt("context_tokens")
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            listOf(2048, 4096, 8192).forEach { v ->
                                FilterChip(selected = ctxTokens == v, onClick = {
                                    scope.launch { bridge.io { setContextTokens(v.toUInt()) }; reload++ }
                                }, label = { Text("${v / 1024}K") })
                            }
                        }
                    }
                }
            }
        }
        item {
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.privacy_title), style = MaterialTheme.typography.titleMedium)
                    Text(stringResource(R.string.privacy_body))
                    Text(stringResource(R.string.core_version, bridge.core.let { uniffi.ara_ffi.coreVersion() }),
                        style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    }
}
