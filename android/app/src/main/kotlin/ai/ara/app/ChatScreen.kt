package ai.ara.app

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject
import uniffi.ara_ffi.CancelHandle
import uniffi.ara_ffi.FfiException

/** A message as displayed. Tool data messages are hidden from the user. */
private data class Bubble(val fromUser: Boolean, val text: String)

private fun visibleBubbles(conv: JSONObject): List<Bubble> =
    conv.getJSONArray("messages").objects().mapNotNull { m ->
        val content = m.getString("content")
        when (m.getString("role")) {
            "user" -> if (content.startsWith("<tool_response>")) null else Bubble(true, content)
            "assistant" -> content.substringBefore("<tool_call>").trim()
                .takeIf { it.isNotEmpty() }?.let { Bubble(false, it) }
            else -> null
        }
    }

@Composable
fun ChatScreen(
    bridge: CoreBridge,
    onNeedModel: () -> Unit,
    prefillText: String? = null,
    speakReplies: Boolean = false,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var convId by rememberSaveable { mutableStateOf<String?>(null) }
    val bubbles = remember { mutableStateListOf<Bubble>() }
    var streaming by remember { mutableStateOf<String?>(null) }
    var cancel by remember { mutableStateOf<CancelHandle?>(null) }
    var input by rememberSaveable(prefillText) { mutableStateOf(prefillText.orEmpty()) }
    var pending by remember { mutableStateOf<JSONObject?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    var showHistory by remember { mutableStateOf(false) }
    var loaded by remember { mutableStateOf<String?>(null) }
    var persona by rememberSaveable { mutableStateOf("general") }
    var personas by remember { mutableStateOf(emptyList<JSONObject>()) }
    val arabic = isArabicLocale()
    val list = rememberLazyListState()

    // Voice I/O — Android's own on-device APIs only, see Voice.kt. The mic
    // button fills `input` for the user to review/edit, same as any other
    // prefilled text in this app; it never sends on its own.
    var listening by remember { mutableStateOf(false) }
    val voiceInput = remember { VoiceInput(ctx) }
    val voiceOutput = remember { VoiceOutput(ctx) }
    val speechLanguageTag = if (arabic) "ar-EG" else "en-US"
    val voiceInputLabel = stringResource(R.string.voice_input_label)
    val voiceListeningLabel = stringResource(R.string.voice_listening)
    val voiceNotAvailableMsg = stringResource(R.string.voice_not_available)
    val voiceNoPermissionMsg = stringResource(R.string.voice_no_permission)
    val voiceNoSpeechMsg = stringResource(R.string.voice_no_speech)
    DisposableEffect(Unit) {
        onDispose {
            voiceInput.stop()
            voiceOutput.shutdown()
        }
    }
    fun startListening() {
        listening = true
        voiceInput.start(
            speechLanguageTag,
            onPartial = { input = it },
            onFinal = { text -> input = text; listening = false },
            onError = { err ->
                listening = false
                notice = when (err) {
                    VoiceError.NotAvailable -> voiceNotAvailableMsg
                    VoiceError.NoPermission -> voiceNoPermissionMsg
                    VoiceError.NoSpeech, VoiceError.Other -> voiceNoSpeechMsg
                }
            },
        )
    }
    val micPermissionLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) startListening() else notice = voiceNoPermissionMsg
    }
    fun onMicTap() {
        if (listening) {
            voiceInput.stop()
            listening = false
        } else if (ctx.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            startListening()
        } else {
            micPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    LaunchedEffect(Unit) { personas = bridge.personas() }
    LaunchedEffect(convId) {
        loaded = bridge.io { loadedModel() }
        val id = convId ?: return@LaunchedEffect
        bubbles.clear()
        runCatching { bridge.conversation(id) }.getOrNull()?.let {
            persona = it.optString("persona", "general")
            bubbles.addAll(visibleBubbles(it))
        }
    }
    LaunchedEffect(bubbles.size, streaming) {
        val n = bubbles.size + (if (streaming != null) 1 else 0)
        if (n > 0) list.animateScrollToItem(n - 1)
    }

    fun send() {
        val text = input.trim()
        if (text.isEmpty() || cancel != null) return
        input = ""
        notice = null
        bubbles.add(Bubble(true, text))
        val handle = CancelHandle()
        cancel = handle
        streaming = ""
        scope.launch {
            try {
                val id = convId ?: bridge.io { createConversation("", persona) }.also { convId = it }
                val r = bridge.send(id, text, handle) { t -> streaming = (streaming ?: "") + t }
                val shown = r.optString("text").ifBlank { streaming.orEmpty() }.trim()
                if (shown.isNotEmpty()) {
                    bubbles.add(Bubble(false, shown))
                    if (speakReplies) voiceOutput.speak(shown, speechLanguageTag)
                }
                pending = r.optJSONObject("pending_confirmation")
                r.optJSONArray("tool_events")?.objects()?.forEach { e ->
                    val o = e.getJSONObject("outcome")
                    if (o.getString("outcome") == "run_on_host") {
                        val c = o.getJSONObject("call")
                        HostActions.run(ctx, c.getString("tool"), c.getJSONObject("arguments"))?.let { notice = it }
                    }
                }
            } catch (e: FfiException.Cancelled) {
                streaming?.takeIf { it.isNotBlank() }?.let { bubbles.add(Bubble(false, it.trim() + " …")) }
            } catch (e: FfiException) {
                notice = e.message
            } finally {
                streaming = null
                cancel = null
            }
        }
    }

    Column(Modifier.fillMaxSize().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.app_name), style = MaterialTheme.typography.titleLarge)
                Text(loaded?.let {
                    if (it.startsWith("cloud:")) stringResource(R.string.running_cloud, it.removePrefix("cloud:"))
                    else stringResource(R.string.running_locally, it)
                } ?: stringResource(R.string.no_model_loaded),
                    style = MaterialTheme.typography.bodySmall,
                    color = if (loaded?.startsWith("cloud:") == true) MaterialTheme.colorScheme.tertiary
                        else MaterialTheme.colorScheme.onSurfaceVariant)
            }
            TextButton({ showHistory = true }) { Text(stringResource(R.string.history)) }
            TextButton({ convId = null; bubbles.clear(); pending = null }) { Text(stringResource(R.string.new_chat)) }
        }
        if (convId == null && personas.isNotEmpty()) {
            androidx.compose.foundation.lazy.LazyRow(
                Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                items(personas) { p ->
                    androidx.compose.material3.FilterChip(
                        selected = persona == p.getString("id"),
                        onClick = { persona = p.getString("id") },
                        label = { Text(p.getString(if (arabic) "name_ar" else "name_en")) },
                    )
                }
            }
        }
        HorizontalDivider()

        AnimatedVisibility(loaded == null, enter = fadeIn(tween(motionMillis())), exit = fadeOut(tween(motionMillis()))) {
            Card(Modifier.fillMaxWidth().padding(12.dp)) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.need_model))
                    Button(onNeedModel) { Text(stringResource(R.string.choose_model)) }
                }
            }
        }

        LazyColumn(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp), state = list,
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(bubbles) { BubbleView(it, Modifier.animateItem()) }
            streaming?.let { s -> item { BubbleView(Bubble(false, s.ifEmpty { "…" })) } }
        }

        AnimatedVisibility(pending != null, enter = fadeIn(tween(motionMillis())), exit = fadeOut(tween(motionMillis()))) {
            val p = pending
            if (p != null) Card(
                Modifier.fillMaxWidth().padding(12.dp),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer),
            ) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.confirm_title), style = MaterialTheme.typography.titleSmall)
                    Text(p.getString("summary"))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button({
                            val call = p.getJSONObject("call").toString()
                            pending = null
                            scope.launch {
                                val d = JSONObject(bridge.io { confirmAction(call, true) })
                                if (d.getString("outcome") == "run_on_host") {
                                    val c = d.getJSONObject("call")
                                    notice = HostActions.run(ctx, c.getString("tool"), c.getJSONObject("arguments"))
                                } else if (d.getString("outcome") == "denied") {
                                    notice = d.optString("reason")
                                }
                            }
                        }) { Text(stringResource(R.string.approve)) }
                        OutlinedButton({ pending = null }) { Text(stringResource(R.string.decline)) }
                    }
                }
            }
        }
        AnimatedVisibility(notice != null, enter = fadeIn(tween(motionMillis())), exit = fadeOut(tween(motionMillis()))) {
            Text(
                notice.orEmpty(),
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = 12.dp).liveRegionAnnounce(),
            )
        }

        if (prefillText != null) {
            Text(
                stringResource(R.string.process_text_prefill_note),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 12.dp),
            )
        }
        Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(input, { input = it }, Modifier.weight(1f),
                placeholder = { Text(stringResource(R.string.message_hint)) }, maxLines = 5)
            TextButton(::onMicTap, Modifier.padding(start = 4.dp), enabled = cancel == null) {
                Text(if (listening) voiceListeningLabel else voiceInputLabel)
            }
            if (cancel != null) OutlinedButton({ cancel?.cancel() }, Modifier.padding(start = 8.dp)) {
                Text(stringResource(R.string.stop))
            } else Button(::send, Modifier.padding(start = 8.dp), enabled = loaded != null && input.isNotBlank()) {
                Text(stringResource(R.string.send))
            }
        }
    }

    if (showHistory) HistoryDialog(bridge, onDismiss = { showHistory = false }) { id ->
        showHistory = false
        pending = null
        convId = id
    }
}

@Composable
private fun BubbleView(b: Bubble, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth(), contentAlignment = if (b.fromUser) Alignment.CenterEnd else Alignment.CenterStart) {
        Text(b.text, Modifier
            .widthIn(max = 320.dp)
            .animateContentSize(tween(motionMillis()))
            .background(
                if (b.fromUser) MaterialTheme.colorScheme.primaryContainer
                else MaterialTheme.colorScheme.surfaceVariant,
                RoundedCornerShape(14.dp))
            .padding(10.dp))
    }
}

@Composable
private fun HistoryDialog(bridge: CoreBridge, onDismiss: () -> Unit, onOpen: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf(emptyList<JSONObject>()) }
    var query by rememberSaveable { mutableStateOf("") }
    var confirmDelete by remember { mutableStateOf<JSONObject?>(null) }
    var reload by remember { mutableStateOf(0) }
    LaunchedEffect(reload) { items = bridge.conversations().objects() }
    val shown = remember(items, query) {
        if (query.isBlank()) items else items.filter { it.getString("title").contains(query, ignoreCase = true) }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = { TextButton(onDismiss) { Text(stringResource(R.string.close)) } },
        title = { Text(stringResource(R.string.history)) },
        text = {
            Column {
                if (items.size > 5) OutlinedTextField(
                    query, { query = it }, Modifier.fillMaxWidth(),
                    placeholder = { Text(stringResource(R.string.search_conversations)) }, singleLine = true,
                )
                if (shown.isEmpty()) Text(stringResource(R.string.no_conversations))
                LazyColumn {
                    items(shown, key = { it.getString("id") }) { c ->
                        Row(
                            Modifier.fillMaxWidth().animateItem().clickable { onOpen(c.getString("id")) }
                                .padding(vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(c.getString("title"), Modifier.weight(1f))
                            TextButton({ confirmDelete = c }) { Text(stringResource(R.string.delete)) }
                        }
                    }
                }
            }
        },
    )

    confirmDelete?.let { c ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text(stringResource(R.string.delete_conversation_title)) },
            text = { Text(stringResource(R.string.delete_conversation_body)) },
            confirmButton = {
                Button({
                    confirmDelete = null
                    scope.launch { bridge.io { deleteConversation(c.getString("id")) }; reload++ }
                }) { Text(stringResource(R.string.delete)) }
            },
            dismissButton = { TextButton({ confirmDelete = null }) { Text(stringResource(R.string.decline)) } },
        )
    }
}
