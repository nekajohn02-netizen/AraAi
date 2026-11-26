package ai.ara.app

import android.text.format.Formatter
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject
import uniffi.ara_ffi.CancelHandle
import uniffi.ara_ffi.FfiException
import java.util.Locale

private data class Dl(val done: Long, val total: Long, val cancel: CancelHandle)

@Composable
fun ModelsScreen(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var mode by remember { mutableStateOf("automatic") }
    var persona by remember { mutableStateOf("general") }
    var personas by remember { mutableStateOf(emptyList<JSONObject>()) }
    val arabic = isArabicLocale()
    var data by remember { mutableStateOf<JSONObject?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    val downloads = remember { mutableStateMapOf<String, Dl>() }
    val lang = currentLanguageTag()
    var confirmDelete by remember { mutableStateOf<String?>(null) }
    var benchmarkCancel by remember { mutableStateOf<CancelHandle?>(null) }

    LaunchedEffect(Unit) { personas = bridge.personas() }
    LaunchedEffect(mode, persona, refresh) {
        error = null
        data = runCatching { bridge.models(mode, lang, persona) }
            .onFailure { error = (it as? FfiException)?.message ?: it.toString() }
            .getOrNull()
    }
    fun bytes(b: Long) = Formatter.formatShortFileSize(ctx, b)
    fun act(id: String, block: suspend () -> Unit) = scope.launch {
        busy = id
        error = null
        try { block() } catch (e: Exception) { error = e.rethrowIfCancellation().message ?: e.toString() }
        busy = null
        refresh++
    }

    LazyColumn(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            Text(stringResource(R.string.models_title), style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(top = 16.dp))
        }
        data?.optJSONObject("device")?.let { d ->
            item {
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(stringResource(R.string.title_device), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(R.string.device_line,
                            bytes(d.getLong("available_ram_bytes")), bytes(d.getLong("total_ram_bytes")),
                            d.getInt("cpu_cores"), d.getString("arch")))
                        val st = data!!.getJSONObject("storage")
                        Text(stringResource(R.string.storage_line,
                            bytes(st.getLong("free_bytes")), bytes(st.getLong("models_bytes"))))
                    }
                }
            }
        }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("automatic" to R.string.mode_auto, "quality" to R.string.mode_quality,
                    "speed" to R.string.mode_speed).forEach { (m, label) ->
                    FilterChip(selected = mode == m, onClick = { mode = m }, label = { Text(stringResource(label)) })
                }
            }
        }
        item {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.recommend_for), style = MaterialTheme.typography.labelLarge)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    personas.forEach { p ->
                        FilterChip(selected = persona == p.getString("id"), onClick = { persona = p.getString("id") },
                            label = { Text(p.getString(if (arabic) "name_ar" else "name_en")) })
                    }
                }
            }
        }
        item {
            AnimatedVisibility(
                error != null, enter = fadeIn(tween(motionMillis())), exit = fadeOut(tween(motionMillis())),
            ) {
                Text(error.orEmpty(), color = MaterialTheme.colorScheme.error, modifier = Modifier.liveRegionAnnounce())
            }
        }
        if (data == null && error == null) item { LinearProgressIndicator(Modifier.fillMaxWidth()) }

        val rows = data?.getJSONArray("models")?.objects().orEmpty()
        items(rows, key = { it.getString("id") }) { m ->
            val id = m.getString("id")
            val cand = m.optJSONObject("candidate")
            val installed = m.getJSONObject("install").getString("state") == "installed"
            val partial = m.getJSONObject("install").optLong("bytes", 0)
            val dl = downloads[id]
            Card(
                Modifier.fillMaxWidth().animateItem(),
                colors = if (m.optBoolean("recommended")) CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer) else CardDefaults.cardColors(),
            ) {
                Column(
                    Modifier.padding(14.dp).animateContentSize(tween(motionMillis())),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Text(m.getString("display_name") + "  ·  " + m.getString("quantization"),
                        style = MaterialTheme.typography.titleMedium)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (m.optBoolean("recommended")) AssistChip({}, { Text(stringResource(R.string.recommended)) })
                        if (m.optBoolean("loaded")) AssistChip({}, { Text(stringResource(R.string.in_use)) })
                        AssistChip({}, { Text(m.getString("license")) })
                        AssistChip({}, { Text(bytes(m.getLong("file_size_bytes"))) })
                    }
                    if (cand != null) {
                        val tps = cand.getDouble("estimated_tokens_per_sec")
                        Text(stringResource(
                            if (cand.getBoolean("estimated")) R.string.fit_estimated else R.string.fit_measured,
                            bytes(cand.getLong("required_memory_bytes")), String.format(Locale.US, "%.0f", tps)))
                    } else {
                        Text(stringResource(R.string.not_suitable) + ": " +
                            reason(m.getJSONObject("excluded")), color = MaterialTheme.colorScheme.error)
                    }
                    if (dl != null) {
                        LinearProgressIndicator(
                            progress = { if (dl.total > 0) dl.done.toFloat() / dl.total else 0f },
                            modifier = Modifier.fillMaxWidth())
                        Text("${bytes(dl.done)} / ${bytes(dl.total)}")
                    }
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        when {
                            dl != null -> OutlinedButton({ dl.cancel.cancel() }) { Text(stringResource(R.string.cancel)) }
                            installed -> {
                                if (!m.optBoolean("loaded")) Button(enabled = busy == null, onClick = {
                                    act(id) { bridge.io { loadModel(id) } }
                                }) { Text(stringResource(R.string.use_model)) }
                                else if (busy == id && benchmarkCancel != null) {
                                    OutlinedButton({ benchmarkCancel?.cancel() }) { Text(stringResource(R.string.cancel)) }
                                } else OutlinedButton(enabled = busy == null, onClick = {
                                    val handle = CancelHandle()
                                    benchmarkCancel = handle
                                    act(id) {
                                        try {
                                            bridge.io { benchmarkLoaded(handle) }
                                        } finally {
                                            benchmarkCancel = null
                                        }
                                    }
                                }) { Text(stringResource(R.string.benchmark)) }
                                TextButton(enabled = busy == null && !m.optBoolean("loaded"), onClick = {
                                    confirmDelete = id
                                }) { Text(stringResource(R.string.delete)) }
                            }
                            cand != null -> Button(onClick = {
                                val cancel = CancelHandle()
                                downloads[id] = Dl(partial, m.getLong("file_size_bytes"), cancel)
                                scope.launch {
                                    try {
                                        bridge.download(id, cancel) { d, t -> downloads[id] = Dl(d, t, cancel) }
                                    } catch (e: FfiException.Cancelled) {
                                    } catch (e: Exception) { error = e.rethrowIfCancellation().message }
                                    downloads.remove(id)
                                    refresh++
                                }
                            }) {
                                Text(stringResource(if (partial > 0) R.string.resume else R.string.download))
                            }
                        }
                    }
                    if (busy == id) LinearProgressIndicator(Modifier.fillMaxWidth())
                }
            }
        }
        item { ApiHubSection(bridge, onChanged = { refresh++ }) }
        item { Text(stringResource(R.string.models_note), style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(bottom = 24.dp)) }
    }

    confirmDelete?.let { id ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text(stringResource(R.string.delete_model_title)) },
            text = { Text(stringResource(R.string.delete_model_body)) },
            confirmButton = {
                Button({
                    confirmDelete = null
                    act(id) { bridge.io { deleteModel(id) } }
                }) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = { TextButton({ confirmDelete = null }) { Text(stringResource(R.string.decline)) } },
        )
    }
}

@Composable
private fun reason(e: JSONObject): String = when (e.getString("reason")) {
    "insufficient_memory" -> stringResource(R.string.why_memory)
    "insufficient_storage" -> stringResource(R.string.why_storage)
    "unsupported_language" -> stringResource(R.string.why_language)
    "context_too_large" -> stringResource(R.string.why_context)
    "unstable_on_device" -> stringResource(R.string.why_unstable)
    else -> e.getString("reason")
}
