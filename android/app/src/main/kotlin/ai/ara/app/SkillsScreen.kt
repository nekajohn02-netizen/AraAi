package ai.ara.app

import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

private const val MAX_SKILL_MANIFEST_BYTES = 256L * 1024

private val GRANTS = listOf(
    "denied" to R.string.grant_denied,
    "ask_every_time" to R.string.grant_ask,
    "always" to R.string.grant_always,
)

@Composable
fun SkillsScreen(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var skills by remember { mutableStateOf(emptyList<JSONObject>()) }
    var grants by remember { mutableStateOf(JSONObject()) }
    var reload by remember { mutableIntStateOf(0) }
    var importError by remember { mutableStateOf<String?>(null) }
    var pendingImport by remember { mutableStateOf<JSONObject?>(null) }
    var confirmRemove by remember { mutableStateOf<String?>(null) }
    val arabic = isArabicLocale()

    LaunchedEffect(reload) {
        skills = bridge.io { JSONArray(skills()) }.objects()
        grants = bridge.io { JSONObject(permissions()) }.optJSONObject("grants") ?: JSONObject()
    }

    // Reads the picked file but does NOT import yet — `import_skill` (Rust)
    // requires the unskippable warning to be shown first (ADR-017/T2.2c);
    // this just parses enough (tools/name) to put in that warning.
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            importError = null
            try {
                // A skill manifest is a small JSON file; reading an unbounded
                // pick (e.g. the user selects a large file by mistake) into
                // memory with readBytes() risked an OOM crash on-device.
                val (name, size) = withContext(Dispatchers.IO) { nameAndSize(ctx, uri) }
                if (size > MAX_SKILL_MANIFEST_BYTES) {
                    importError = "$name: >256 KB"
                    return@launch
                }
                val text = withContext(Dispatchers.IO) {
                    ctx.contentResolver.openInputStream(uri)?.use { it.readBytes() }
                        ?.toString(Charsets.UTF_8) ?: throw java.io.IOException("empty file")
                }
                pendingImport = JSONObject(text)
            } catch (e: Exception) {
                importError = e.rethrowIfCancellation().message
            }
        }
    }

    LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.skills_title), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(R.string.skills_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        item {
            OutlinedButton({ picker.launch(arrayOf("application/json")) }) {
                Text(stringResource(R.string.import_skill))
            }
        }
        item {
            AnimatedVisibility(
                importError != null, enter = fadeIn(tween(motionMillis())), exit = fadeOut(tween(motionMillis())),
            ) {
                Text(
                    importError.orEmpty(), color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.liveRegionAnnounce(),
                )
            }
        }
        items(skills, key = { it.getJSONObject("skill").getString("id") }) { row ->
            val s = row.getJSONObject("skill")
            val builtIn = s.optString("trust_tier", "built_in") == "built_in"
            Card(Modifier.fillMaxWidth().animateItem()) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(s.getString(if (arabic) "name_ar" else "name_en"), style = MaterialTheme.typography.titleMedium)
                            Text(s.getString("description"), style = MaterialTheme.typography.bodySmall)
                        }
                        if (!builtIn) {
                            Text(stringResource(R.string.skill_imported_tag), style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.tertiary)
                        }
                    }
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
                    if (!builtIn) {
                        TextButton({ confirmRemove = s.getString("id") }) { Text(stringResource(R.string.delete)) }
                    }
                }
            }
        }
        item { Text(stringResource(R.string.external_always_confirm), style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(bottom = 24.dp)) }
    }

    // ADR-017/T2.2c: this warning is not skippable with one tap and must
    // name the requested tools before any import call happens.
    pendingImport?.let { manifest ->
        val tools = manifest.optJSONArray("tools")?.strings().orEmpty()
        AlertDialog(
            onDismissRequest = { pendingImport = null },
            title = { Text(stringResource(R.string.import_skill_warning_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.import_skill_warning_body))
                    Text(
                        stringResource(R.string.import_skill_tools_requested, tools.joinToString(", ").ifEmpty { "—" }),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            },
            confirmButton = {
                Button({
                    val json = manifest.toString()
                    pendingImport = null
                    scope.launch {
                        importError = try {
                            bridge.io { importSkill(json) }
                            reload++
                            null
                        } catch (e: Exception) {
                            e.rethrowIfCancellation().message
                        }
                    }
                }) { Text(stringResource(R.string.import_skill_confirm)) }
            },
            dismissButton = { TextButton({ pendingImport = null }) { Text(stringResource(R.string.decline)) } },
        )
    }

    confirmRemove?.let { id ->
        AlertDialog(
            onDismissRequest = { confirmRemove = null },
            title = { Text(stringResource(R.string.delete_skill_title)) },
            text = { Text(stringResource(R.string.delete_skill_body)) },
            confirmButton = {
                Button({
                    confirmRemove = null
                    scope.launch { bridge.io { deleteImportedSkill(id) }; reload++ }
                }) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = { TextButton({ confirmRemove = null }) { Text(stringResource(R.string.decline)) } },
        )
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
    "smart_home" -> R.string.perm_smart_home
    else -> R.string.perm_files
})

@Composable
fun SettingsScreen(
    bridge: CoreBridge,
    reduceMotion: Boolean,
    onReduceMotionChange: (Boolean) -> Unit,
    speakReplies: Boolean,
    onSpeakRepliesChange: (Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var storage by remember { mutableStateOf<JSONObject?>(null) }
    var strictOffline by remember { mutableStateOf(false) }
    var reload by remember { mutableIntStateOf(0) }
    LaunchedEffect(reload) {
        storage = bridge.io { JSONObject(storageInfo()) }
        strictOffline = bridge.io { strictOffline() }
    }
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
        item {
            Card(Modifier.fillMaxWidth()) {
                Row(
                    Modifier.fillMaxWidth().padding(14.dp),
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.strict_offline_title), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.strict_offline_note), style = MaterialTheme.typography.bodySmall)
                    }
                    Switch(strictOffline, { on ->
                        strictOffline = on
                        scope.launch { bridge.io { setStrictOffline(on) } }
                    })
                }
            }
        }
        item { SmartHomeSection(bridge) }
        item {
            Card(Modifier.fillMaxWidth()) {
                Row(
                    Modifier.fillMaxWidth().padding(14.dp),
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.reduce_motion_title), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.reduce_motion_note), style = MaterialTheme.typography.bodySmall)
                    }
                    Switch(reduceMotion, onReduceMotionChange)
                }
            }
        }
        item {
            Card(Modifier.fillMaxWidth()) {
                Row(
                    Modifier.fillMaxWidth().padding(14.dp),
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.speak_replies_title), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.speak_replies_note), style = MaterialTheme.typography.bodySmall)
                    }
                    Switch(speakReplies, onSpeakRepliesChange)
                }
            }
        }
        item { Text("", Modifier.padding(bottom = 24.dp)) }
    }
}
