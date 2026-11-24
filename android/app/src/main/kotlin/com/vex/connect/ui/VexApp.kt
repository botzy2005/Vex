package com.vex.connect.ui

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.vex.connect.AppContainer
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.navigation.MainScaffold
import com.vex.connect.ui.launch.LaunchScreen
import com.vex.connect.ui.onboarding.OnboardingScreen
import kotlinx.coroutines.launch

@Composable
fun VexApp(container: AppContainer, onSystemBars: (dark: Boolean) -> Unit) {
    val factory = remember(container) { AppViewModelFactory(container) }
    val rootVm: RootViewModel = viewModel(factory = factory)
    val settings by rootVm.settings.collectAsStateWithLifecycle()
    var launchFinished by rememberSaveable { mutableStateOf(false) }

    val dark = isDarkTheme(settings?.themeMode ?: com.vex.connect.core.domain.model.ThemeMode.Dark, isSystemInDarkTheme())
    DisposableEffect(dark) {
        onSystemBars(dark)
        onDispose { }
    }

    VexTheme(darkTheme = dark) {
        val stage = resolveStage(launchFinished, settings != null, settings?.onboardingDone ?: false)
        Crossfade(targetState = stage, animationSpec = tween(320), label = "stage") { current ->
            when (current) {
                Stage.Launch -> LaunchScreen(onFinished = { launchFinished = true })
                Stage.Onboarding -> {
                    val scope = rememberCoroutineScope()
                    OnboardingScreen(onDone = { scope.launch { rootVm.finishOnboarding() } })
                }
                Stage.Main -> MainScaffold(factory = factory)
            }
        }
    }
}
