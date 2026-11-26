package ai.ara.app

import android.Manifest
import android.content.ComponentCallbacks2
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    /** Keeps Android's native splash on screen until [CoreBridge] has opened
     *  its on-disk store, so the first Compose frame is already usable
     *  instead of showing an empty window while that I/O runs. */
    private var coreReady = false

    /** Latest "Ask Ara" text-selection payload, observed by the Compose
     *  tree. A plain `val` captured once in `onCreate` would miss a second
     *  "Ask Ara" tap while Ara is already running: `singleTask` (manifest)
     *  reuses this same Activity instance via [onNewIntent] instead of
     *  creating a new one, so the value has to be reactive, not one-shot. */
    private var prefillText by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        splash.setKeepOnScreenCondition { !coreReady }
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        prefillText = processTextFrom(intent)
        setContent {
            AraTheme {
                var bridge by remember { mutableStateOf<CoreBridge?>(null) }
                var startupError by remember { mutableStateOf<String?>(null) }
                var retry by remember { mutableIntStateOf(0) }
                LaunchedEffect(retry) {
                    // Catching `Throwable`, not `Exception`: a native-library
                    // load failure (e.g. no matching ABI for this device,
                    // the real cause found 2026-11-26 on a 32-bit-only
                    // tablet) surfaces as `UnsatisfiedLinkError`/
                    // `ExceptionInInitializerError`, both `Error` subtypes a
                    // bare `catch (e: Exception)` would miss entirely —
                    // leaving this coroutine's exception uncaught, which
                    // crashes the whole app with no on-screen explanation.
                    try {
                        bridge = CoreBridge.getAsync(this@MainActivity)
                    } catch (t: Throwable) {
                        startupError = "${t::class.java.simpleName}: ${t.message ?: "unknown startup error"}"
                    } finally {
                        coreReady = true
                    }
                }
                val b = bridge
                val err = startupError
                when {
                    err != null -> StartupErrorScreen(err) { startupError = null; coreReady = false; retry++ }
                    b == null -> {
                        // Same brand background as the native splash so there is
                        // no visible seam while the store finishes opening.
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            Image(
                                painterResource(R.mipmap.ic_launcher_foreground),
                                contentDescription = null,
                                modifier = Modifier.size(96.dp),
                            )
                        }
                    }
                    else -> AraApp(b, prefillText)
                }
            }
        }
    }

    /** `singleTask` (manifest) routes a second launch here instead of
     *  starting a new instance — e.g. a second "Ask Ara" tap, or the
     *  launcher icon tapped while Ara is already open. Without this
     *  override that second launch would just bring the existing window
     *  forward with none of its own intent's data (a new selected text)
     *  ever read, since `onCreate` does not run again. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        processTextFrom(intent)?.let { prefillText = it }
    }

    @Composable
    private fun StartupErrorScreen(error: String, onRetry: () -> Unit) {
        Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(stringResource(R.string.startup_error_title), style = MaterialTheme.typography.titleMedium)
                Text(error, style = MaterialTheme.typography.bodySmall)
                Button(onRetry) { Text(stringResource(R.string.startup_error_retry)) }
            }
        }
    }

    private fun processTextFrom(intent: Intent?): String? = intent
        ?.takeIf { it.action == Intent.ACTION_PROCESS_TEXT }
        ?.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)
        ?.toString()
        ?.takeIf { it.isNotBlank() }

    /** Free the model's RAM when Android asks for memory. Runs off the main
     *  thread: this calls into the native engine, which must never block UI. */
    @Deprecated("Still delivered on all supported API levels")
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND) {
            lifecycleScope.launch(Dispatchers.IO) {
                runCatching { CoreBridge.get(this@MainActivity).core.unloadModel() }
            }
        }
    }
}

@Composable
fun AraTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val ctx = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        dark -> darkColorScheme()
        else -> lightColorScheme()
    }
    MaterialTheme(colorScheme = scheme, content = content)
}

private enum class Tab(val label: Int, val icon: Int) {
    Chat(R.string.tab_chat, android.R.drawable.sym_action_chat),
    Files(R.string.tab_files, android.R.drawable.ic_menu_search),
    Models(R.string.tab_models, android.R.drawable.ic_menu_manage),
    Missions(R.string.tab_missions, android.R.drawable.ic_menu_recent_history),
    Skills(R.string.tab_skills, android.R.drawable.ic_menu_agenda),
    Security(R.string.tab_security, android.R.drawable.ic_lock_lock),
    Settings(R.string.tab_settings, android.R.drawable.ic_menu_preferences),
}

@Composable
private fun AraApp(bridge: CoreBridge, prefillText: String? = null) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val ctx = LocalContext.current
    val notifLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    var reduceMotionPref by remember { mutableStateOf(readReduceMotionPref(ctx)) }
    var speakRepliesPref by remember { mutableStateOf(readSpeakRepliesPref(ctx)) }
    val reduceMotion = reduceMotionPref || systemRemovesAnimations()
    val tabTransitionMillis = if (reduceMotion) 0 else 150

    // Resume missions left running across an app close/reboot, and ask
    // (once) for the permission that lets their notifications show. Both
    // are best-effort: a mission that doesn't resume yet is still safe —
    // nothing executes without the confirmation it was already waiting on.
    LaunchedEffect(Unit) {
        runCatching { bridge.io { resumableMissions() } }.getOrNull()?.forEach { id ->
            MissionWorker.enqueueNext(ctx, id)
        }
        SchedulerWorker.ensureScheduled(ctx)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ctx.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    CompositionLocalProvider(LocalReduceMotion provides reduceMotion) {
        Scaffold(
            bottomBar = {
                NavigationBar {
                    Tab.entries.forEachIndexed { i, t ->
                        NavigationBarItem(
                            selected = tab == i,
                            onClick = { tab = i },
                            icon = { Icon(painterResource(t.icon), contentDescription = null) },
                            label = { Text(stringResource(t.label)) },
                        )
                    }
                }
            },
        ) { pad ->
            Box(Modifier.fillMaxSize().padding(pad)) {
                AnimatedContent(
                    targetState = tab,
                    transitionSpec = {
                        fadeIn(tween(tabTransitionMillis)) togetherWith fadeOut(tween(tabTransitionMillis))
                    },
                    label = "tab",
                ) { i ->
                    when (Tab.entries[i]) {
                        Tab.Chat -> ChatScreen(
                            bridge,
                            onNeedModel = { tab = Tab.Models.ordinal },
                            prefillText = prefillText,
                            speakReplies = speakRepliesPref,
                        )
                        Tab.Files -> FilesScreen(bridge)
                        Tab.Models -> ModelsScreen(bridge)
                        Tab.Missions -> MissionsScreen(bridge)
                        Tab.Skills -> SkillsScreen(bridge)
                        Tab.Security -> SecurityScreen(bridge)
                        Tab.Settings -> SettingsScreen(
                            bridge,
                            reduceMotion = reduceMotionPref,
                            onReduceMotionChange = { reduceMotionPref = it; writeReduceMotionPref(ctx, it) },
                            speakReplies = speakRepliesPref,
                            onSpeakRepliesChange = { speakRepliesPref = it; writeSpeakRepliesPref(ctx, it) },
                        )
                    }
                }
            }
        }
    }
}
