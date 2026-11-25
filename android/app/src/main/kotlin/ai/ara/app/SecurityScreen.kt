package ai.ara.app

import android.app.KeyguardManager
import android.content.Context
import android.content.pm.ApplicationInfo
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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

@Composable
fun SecurityScreen(bridge: CoreBridge) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var report by remember { mutableStateOf<JSONObject?>(null) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    fun scan() {
        busy = true
        error = null
        scope.launch {
            try {
                val facts = collectHostSecurityFacts(ctx).toString()
                report = bridge.io { JSONObject(securityCheck(facts)) }
            } catch (e: Exception) {
                error = e.message
            } finally {
                busy = false
            }
        }
    }

    LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.security_title), style = MaterialTheme.typography.headlineSmall)
                Text(stringResource(R.string.security_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        item {
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Button(enabled = !busy, onClick = { scan() }) { Text(stringResource(R.string.run_scan)) }
                if (busy) CircularProgressIndicator(Modifier.padding(start = 4.dp))
            }
        }
        error?.let { item { Text(it, color = MaterialTheme.colorScheme.error) } }
        report?.let { r ->
            item {
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(stringResource(R.string.security_score, r.getInt("score")), style = MaterialTheme.typography.titleMedium)
                        Text(
                            stringResource(R.string.security_checks_run, r.getJSONArray("checks_run").strings().joinToString(", ")),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
            val findings = r.getJSONArray("findings").objects()
            if (findings.isEmpty()) {
                item { Text(stringResource(R.string.security_no_findings)) }
            }
            items(findings) { f ->
                val arabic = java.util.Locale.getDefault().language == "ar"
                val severity = f.getString("severity")
                Card(
                    Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(containerColor = severityColor(severity)),
                ) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(f.getString(if (arabic) "title_ar" else "title_en"), style = MaterialTheme.typography.titleSmall)
                        val fix = f.getString("fix_ar")
                        if (fix.isNotBlank() && arabic) Text(fix, style = MaterialTheme.typography.bodySmall)
                        Text(
                            stringResource(R.string.security_evidence, f.getString("evidence")),
                            style = MaterialTheme.typography.labelSmall,
                        )
                    }
                }
            }
        }
        item { Text("", Modifier.padding(bottom = 24.dp)) }
    }
}

@Composable
private fun severityColor(s: String) = when (s) {
    "high" -> MaterialTheme.colorScheme.errorContainer
    "medium" -> MaterialTheme.colorScheme.tertiaryContainer
    "low" -> MaterialTheme.colorScheme.secondaryContainer
    else -> MaterialTheme.colorScheme.surfaceVariant
}

/**
 * Facts ARA's Rust Sentinel cannot know by itself — gathered from public
 * Android APIs only, never invasive (no root shell, no exploits). Matches
 * `ara_sentinel::HostSecurityFacts`; every field is best-effort and left
 * out (not guessed) when it can't be read.
 */
private fun collectHostSecurityFacts(ctx: Context): JSONObject {
    val j = JSONObject()
    j.put("app_debuggable", (ctx.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0)
    runCatching {
        j.put("adb_enabled", Settings.Global.getInt(ctx.contentResolver, Settings.Global.ADB_ENABLED, 0) == 1)
    }
    runCatching {
        j.put(
            "developer_options",
            Settings.Global.getInt(ctx.contentResolver, Settings.Global.DEVELOPMENT_SETTINGS_ENABLED, 0) == 1,
        )
    }
    runCatching {
        val km = ctx.getSystemService(KeyguardManager::class.java)
        j.put("screen_lock_set", km?.isDeviceSecure)
    }
    j.put("root_indicators", rootIndicators())
    j.put("security_patch", Build.VERSION.SECURITY_PATCH)
    j.put("installed_from_store", installedFromStore(ctx))
    j.put("emulator", looksLikeEmulator())
    return j
}

private val SU_PATHS = listOf(
    "/system/bin/su", "/system/xbin/su", "/sbin/su",
    "/system/app/Superuser.apk", "/system/bin/.ext/.su",
)

private fun rootIndicators(): org.json.JSONArray {
    val found = org.json.JSONArray()
    if (Build.TAGS?.contains("test-keys") == true) found.put("test-keys build")
    SU_PATHS.forEach { p -> if (java.io.File(p).exists()) found.put(p) }
    return found
}

private fun installedFromStore(ctx: Context): Boolean? {
    val installer = runCatching {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            ctx.packageManager.getInstallSourceInfo(ctx.packageName).installingPackageName
        } else {
            @Suppress("DEPRECATION")
            ctx.packageManager.getInstallerPackageName(ctx.packageName)
        }
    }.getOrNull() ?: return null
    return installer == "com.android.vending"
}

private fun looksLikeEmulator(): Boolean {
    val fp = Build.FINGERPRINT.lowercase()
    val model = Build.MODEL.lowercase()
    return fp.contains("generic") || fp.contains("emulator") || fp.startsWith("sdk_gphone") ||
        model.contains("sdk") || model.contains("emulator") || model.contains("genymotion") ||
        Build.MANUFACTURER.lowercase() == "genymotion" || Build.HARDWARE.lowercase().contains("goldfish") ||
        Build.HARDWARE.lowercase().contains("ranchu")
}
