package ai.ara.app

import android.net.Uri
import android.provider.OpenableColumns
import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** Same limit as the Rust index (ara-search MAX_FILE_BYTES). */
private const val MAX_IMPORT_BYTES = 20L * 1024 * 1024

@Composable
fun FilesScreen(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var docs by remember { mutableStateOf(emptyList<JSONObject>()) }
    var mems by remember { mutableStateOf(emptyList<JSONObject>()) }
    var hits by remember { mutableStateOf(emptyList<JSONObject>()) }
    var query by remember { mutableStateOf("") }
    var newMemory by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var message by remember { mutableStateOf<String?>(null) }
    var reload by remember { mutableIntStateOf(0) }

    LaunchedEffect(reload) {
        docs = bridge.io { JSONArray(documents()) }.objects()
        mems = bridge.io { JSONArray(memories()) }.objects()
    }

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        scope.launch {
            busy = true
            message = null
            val errors = mutableListOf<String>()
            for (uri in uris) {
                val (name, size) = withContext(Dispatchers.IO) { nameAndSize(ctx, uri) }
                if (size > MAX_IMPORT_BYTES) { errors += "$name: >20 MB"; continue }
                try {
                    val bytes = withContext(Dispatchers.IO) {
                        ctx.contentResolver.openInputStream(uri)?.use { it.readBytes() }
                    } ?: continue
                    bridge.io { importDocument(name, bytes) }
                } catch (e: Exception) {
                    errors += "$name: ${e.message}"
                }
            }
            message = errors.joinToString("\n").ifEmpty { null }
            busy = false
            reload++
        }
    }

    LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.files_title), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(R.string.files_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        item {
            Button(enabled = !busy, onClick = {
                picker.launch(arrayOf("text/*", "application/json", "text/csv", "text/markdown", "text/html", "application/pdf"))
            }) { Text(stringResource(R.string.import_files)) }
        }
        if (busy) item { LinearProgressIndicator(Modifier.fillMaxWidth()) }
        message?.let { item { Text(it, color = MaterialTheme.colorScheme.error) } }

        item {
            OutlinedTextField(query, {
                query = it
                scope.launch { hits = bridge.io { JSONArray(searchDocuments(it)) }.objects() }
            }, Modifier.fillMaxWidth(), placeholder = { Text(stringResource(R.string.search_files)) }, singleLine = true)
        }
        if (query.isNotBlank() && hits.isEmpty()) item { Text(stringResource(R.string.no_results)) }
        items(hits) { h ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(12.dp)) {
                    Text("${h.getString("document")} · ${stringResource(R.string.line_n, h.getInt("line"))}",
                        style = MaterialTheme.typography.labelLarge)
                    Text(h.getString("text").take(300), style = MaterialTheme.typography.bodySmall)
                }
            }
        }

        item { Text(stringResource(R.string.my_documents), style = MaterialTheme.typography.titleMedium) }
        if (docs.isEmpty()) item { Text(stringResource(R.string.no_documents)) }
        items(docs, key = { "d" + it.getLong("id") }) { d ->
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(d.getString("name"))
                    Text(Formatter.formatShortFileSize(ctx, d.getLong("bytes")), style = MaterialTheme.typography.bodySmall)
                }
                TextButton({ scope.launch { bridge.io { deleteDocument(d.getLong("id")) }; reload++ } }) {
                    Text(stringResource(R.string.delete))
                }
            }
        }

        item {
            Column(Modifier.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.memory_title), style = MaterialTheme.typography.titleMedium)
                Text(stringResource(R.string.memory_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(newMemory, { newMemory = it }, Modifier.weight(1f),
                    placeholder = { Text(stringResource(R.string.memory_hint)) }, singleLine = true)
                TextButton(enabled = newMemory.isNotBlank(), onClick = {
                    val t = newMemory
                    newMemory = ""
                    scope.launch { bridge.io { addMemory(t) }; reload++ }
                }) { Text(stringResource(R.string.add)) }
            }
        }
        items(mems, key = { "m" + it.getLong("id") }) { m ->
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(m.getString("text"), Modifier.weight(1f))
                Switch(m.getBoolean("enabled"), { on ->
                    scope.launch { bridge.io { setMemoryEnabled(m.getLong("id"), on) }; reload++ }
                })
                TextButton({ scope.launch { bridge.io { deleteMemory(m.getLong("id")) }; reload++ } }) {
                    Text(stringResource(R.string.delete))
                }
            }
        }
        item { Text("", Modifier.padding(bottom = 24.dp)) }
    }
}

private fun nameAndSize(ctx: android.content.Context, uri: Uri): Pair<String, Long> {
    ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)
        ?.use { c ->
            if (c.moveToFirst()) {
                val name = c.getString(0) ?: "file.txt"
                val size = if (c.isNull(1)) 0L else c.getLong(1)
                return name to size
            }
        }
    return (uri.lastPathSegment ?: "file.txt") to 0L
}
