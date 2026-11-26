package ai.ara.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

private val EVENT_KINDS = listOf(
    "battery_low" to R.string.event_kind_battery_low,
    "power_connected" to R.string.event_kind_power_connected,
    "power_disconnected" to R.string.event_kind_power_disconnected,
    "network_available" to R.string.event_kind_network_available,
    "network_lost" to R.string.event_kind_network_lost,
)

private fun kindLabelRes(kind: String): Int = EVENT_KINDS.firstOrNull { it.first == kind }?.second ?: R.string.event_kind_battery_low

/**
 * Event Watcher: "when X happens on this device, run this mission."
 * Self-contained — owns its own loading/dialog state — so it can be
 * dropped into any screen as a single composable.
 */
@Composable
fun EventRulesSection(bridge: CoreBridge) {
    val scope = rememberCoroutineScope()
    var rules by remember { mutableStateOf(emptyList<JSONObject>()) }
    var showCreate by remember { mutableStateOf(false) }
    var reload by remember { mutableIntStateOf(0) }
    LaunchedEffect(reload) { rules = bridge.io { JSONArray(eventRules()) }.objects() }

    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.event_rules_title), style = MaterialTheme.typography.titleMedium)
        Text(stringResource(R.string.event_rules_note), style = MaterialTheme.typography.bodySmall)
        rules.forEach { r ->
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(r.getString("goal"), style = MaterialTheme.typography.bodyMedium)
                    Text(stringResource(kindLabelRes(r.getString("kind"))), style = MaterialTheme.typography.labelMedium)
                    TextButton({
                        scope.launch { bridge.io { deleteEventRule(r.getString("id")) }; reload++ }
                    }) { Text(stringResource(R.string.cancel)) }
                }
            }
        }
        TextButton({ showCreate = true }) { Text(stringResource(R.string.new_event_rule)) }
    }

    if (showCreate) {
        var kind by remember { mutableStateOf(EVENT_KINDS[0].first) }
        var goal by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { showCreate = false },
            title = { Text(stringResource(R.string.new_event_rule)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    EVENT_KINDS.forEach { (k, labelRes) ->
                        FilterChip(selected = kind == k, onClick = { kind = k }, label = { Text(stringResource(labelRes)) })
                    }
                    OutlinedTextField(goal, { goal = it }, placeholder = { Text(stringResource(R.string.mission_goal_hint)) })
                }
            },
            confirmButton = {
                Button(
                    enabled = goal.isNotBlank(),
                    onClick = {
                        showCreate = false
                        scope.launch {
                            bridge.io { createEventRule(kind, goal.trim(), "general", 20u, 30u) }
                            reload++
                        }
                    },
                ) { Text(stringResource(R.string.start_mission)) }
            },
            dismissButton = { TextButton({ showCreate = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}
