package com.vex.connect.ui

import com.vex.connect.core.domain.model.ThemeMode

/** Whether the dark theme applies, given the user's choice and the phone's current setting. */
fun isDarkTheme(mode: ThemeMode, systemDark: Boolean): Boolean = when (mode) {
    ThemeMode.System -> systemDark
    ThemeMode.Dark -> true
    ThemeMode.Light -> false
}

/** The three top-level stages of the app, in order. */
enum class Stage { Launch, Onboarding, Main }

/** Launch is shown until both the entrance animation has finished and the saved settings have loaded. */
fun resolveStage(launchFinished: Boolean, settingsLoaded: Boolean, onboardingDone: Boolean): Stage = when {
    !launchFinished || !settingsLoaded -> Stage.Launch
    !onboardingDone -> Stage.Onboarding
    else -> Stage.Main
}
