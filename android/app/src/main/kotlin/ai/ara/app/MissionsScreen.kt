package ai.ara.app

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
import androidx.compose.material3.OutlinedTextField
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
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

private val STEP_PRESETS = listOf(20, 50, 100)
private val MINUTE_PRESETS = listOf(30, 60, 240, 1440) // up to 24h

@Composable
fun MissionsScreen(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var missions by remember { mutableStateOf(emptyList<JSONObject>()) }
    var personas by remember { mutableStateOf(emptyList<JSONObject>()) }
    var showCreate by remember { mutableStateOf(false) }
    var auditFor by remember { mutableStateOf<String?>(null) }
    var reload by remember { mutableIntStateOf(0) }
    val arabic = Locale.getDefault().language == "ar"

    LaunchedEffect(Unit) { personas = bridge.personas() }
    LaunchedEffect(reload) { missions = bridge.io { JSONArray(missions()) }.objects() }

    fun act(block: suspend () -> Unit) {
        scope.launch { block(); reload++ }
    }

    LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.missions_title), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(R.string.missions_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        item { Button({ showCreate = true }) { Text(stringResource(R.string.new_mission)) } }
        if (missions.isEmpty()) item { Text(stringResource(R.string.no_missions)) }
        items(missions, key = { it.getString("id") }) { m ->
            MissionCard(
                m = m,
                onPause = { act { bridge.io { setMissionStatus(m.getString("id"), "paused") }; MissionWorker.cancel(ctx, m.getString("id")) } },
                onResume = { act { bridge.io { setMissionStatus(m.getString("id"), "running") }; MissionWorker.enqueueNext(ctx, m.getString("id")) } },
                onCancel = { act { bridge.io { setMissionStatus(m.getString("id"), "cancelled") }; MissionWorker.cancel(ctx, m.getString("id")) } },
                onShowLog = { auditFor = m.getString("id") },
                onDecide = { approvalId, approve ->
                    act {
                        val callJson = bridge.io { decideMissionApproval(m.getString("id"), approvalId.toUInt(), approve) }
                        if (callJson != "null") {
                            val d = JSONObject(bridge.io { confirmAction(callJson, true) })
                            if (d.getString("outcome") == "run_on_host") {
                                val c = d.getJSONObject("call")
                                HostActions.run(ctx, c.getString("tool"), c.getJSONObject("arguments"))
                            }
                        }
                        if (approve) MissionWorker.enqueueNext(ctx, m.getString("id"))
                    }
                },
            )
        }
        item { Text("", Modifier.padding(bottom = 24.dp)) }
    }

    if (showCreate) CreateMissionDialog(
        personas = personas,
        arabic = arabic,
        onDismiss = { showCreate = false },
        onCreate = { goal, persona, steps, minutes ->
            showCreate = false
            act {
                val id = bridge.io { JSONObject(createMission(goal, persona, steps.toUInt(), minutes.toUInt())) }
                    .getString("id")
                MissionWorker.enqueueNext(ctx, id)
            }
        },
    )

    auditFor?.let { id ->
        AuditDialog(bridge, id, onDismiss = { auditFor = null })
    }
}

@Composable
private fun MissionCard(
    m: JSONObject,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onCancel: () -> Unit,
    onShowLog: () -> Unit,
    onDecide: (Int, Boolean) -> Unit,
) {
    val status = m.getString("status")
    val pending = m.getJSONArray("pending").objects()
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(m.getString("goal"), style = MaterialTheme.typography.titleMedium)
            Text(
                stringResource(R.string.mission_status_line, statusLabel(status), m.getInt("steps_done"), m.getJSONObject("budget").getInt("max_steps")),
                style = MaterialTheme.typography.bodySmall,
            )
            val progress = m.optString("progress")
            if (progress.isNotBlank()) Text(progress, style = MaterialTheme.typography.bodyMedium)
            m.optString("result").takeIf { it.isNotBlank() }?.let {
                Text(stringResource(R.string.mission_result, it), style = MaterialTheme.typography.bodyMedium)
            }
            pending.forEach { a ->
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(a.getString("summary"), style = MaterialTheme.typography.bodyMedium)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button({ onDecide(a.getInt("id"), true) }) { Text(stringResource(R.string.approve)) }
                            OutlinedButton({ onDecide(a.getInt("id"), false) }) { Text(stringResource(R.string.decline)) }
                        }
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                when (status) {
                    "running" -> TextButton(onPause) { Text(stringResource(R.string.pause)) }
                    "paused" -> TextButton(onResume) { Text(stringResource(R.string.resume_mission)) }
                }
                if (status == "running" || status == "paused") {
                    TextButton(onCancel) { Text(stringResource(R.string.cancel)) }
                }
                TextButton(onShowLog) { Text(stringResource(R.string.mission_log)) }
            }
        }
    }
}

@Composable
private fun statusLabel(s: String): String = stringResource(
    when (s) {
        "running" -> R.string.mission_status_running
        "paused" -> R.string.mission_status_paused
        "completed" -> R.string.mission_status_completed
        "failed" -> R.string.mission_status_failed
        "cancelled" -> R.string.mission_status_cancelled
        else -> R.string.mission_status_budget_exhausted
    },
)

@Composable
private fun CreateMissionDialog(
    personas: List<JSONObject>,
    arabic: Boolean,
    onDismiss: () -> Unit,
    onCreate: (goal: String, persona: String, steps: Int, minutes: Int) -> Unit,
) {
    var goal by remember { mutableStateOf("") }
    var persona by remember { mutableStateOf("general") }
    var steps by remember { mutableStateOf(STEP_PRESETS[1]) }
    var minutes by remember { mutableStateOf(MINUTE_PRESETS[1]) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.new_mission)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedTextField(goal, { goal = it }, placeholder = { Text(stringResource(R.string.mission_goal_hint)) })
                if (personas.isNotEmpty()) {
                    Text(stringResource(R.string.mission_persona), style = MaterialTheme.typography.labelLarge)
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        personas.forEach { p ->
                            FilterChip(
                                selected = persona == p.getString("id"),
                                onClick = { persona = p.getString("id") },
                                label = { Text(p.getString(if (arabic) "name_ar" else "name_en")) },
                            )
                        }
                    }
                }
                Text(stringResource(R.string.mission_max_steps), style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    STEP_PRESETS.forEach { v ->
                        FilterChip(selected = steps == v, onClick = { steps = v }, label = { Text("$v") })
                    }
                }
                Text(stringResource(R.string.mission_max_time), style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    MINUTE_PRESETS.forEach { v ->
                        FilterChip(
                            selected = minutes == v,
                            onClick = { minutes = v },
                            label = { Text(if (v >= 60) "${v / 60}h" else "${v}m") },
                        )
                    }
                }
            }
        },
        confirmButton = {
            Button(enabled = goal.isNotBlank(), onClick = { onCreate(goal.trim(), persona, steps, minutes) }) {
                Text(stringResource(R.string.start_mission))
            }
        },
        dismissButton = { TextButton(onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun AuditDialog(bridge: CoreBridge, id: String, onDismiss: () -> Unit) {
    var entries by remember { mutableStateOf(emptyList<JSONObject>()) }
    LaunchedEffect(id) { entries = bridge.io { JSONArray(missionAudit(id)) }.objects() }
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = { TextButton(onDismiss) { Text(stringResource(R.string.close)) } },
        title = { Text(stringResource(R.string.mission_log)) },
        text = {
            if (entries.isEmpty()) Text(stringResource(R.string.no_log_entries))
            LazyColumn {
                items(entries) { e ->
                    Text(
                        "${e.optString("event")}: ${e.optString("detail")}",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(vertical = 3.dp),
                    )
                }
            }
        },
    )
}
