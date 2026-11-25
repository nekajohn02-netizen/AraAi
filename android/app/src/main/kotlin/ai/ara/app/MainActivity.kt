package ai.ara.app

import android.content.ComponentCallbacks2
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val bridge = CoreBridge.get(this)
        setContent { AraTheme { AraApp(bridge) } }
    }

    /** Free the model's RAM when Android asks for memory. */
    @Deprecated("Still delivered on all supported API levels")
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND) {
            CoreBridge.get(this).core.unloadModel()
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
    Skills(R.string.tab_skills, android.R.drawable.ic_menu_agenda),
    Settings(R.string.tab_settings, android.R.drawable.ic_menu_preferences),
}

@Composable
private fun AraApp(bridge: CoreBridge) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
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
            when (Tab.entries[tab]) {
                Tab.Chat -> ChatScreen(bridge, onNeedModel = { tab = Tab.Models.ordinal })
                Tab.Files -> FilesScreen(bridge)
                Tab.Models -> ModelsScreen(bridge)
                Tab.Skills -> SkillsScreen(bridge)
                Tab.Settings -> SettingsScreen(bridge)
            }
        }
    }
}
