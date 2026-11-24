package com.vex.connect.core.designsystem.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf

private val LocalExtras = staticCompositionLocalOf { DarkExtras }

/** Access to the extra VEX colours: `VexTheme.extras.ok`. */
object VexTheme {
    val extras: VexExtras
        @Composable @ReadOnlyComposable get() = LocalExtras.current
}

@Composable
fun VexTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(LocalExtras provides if (darkTheme) DarkExtras else LightExtras) {
        MaterialTheme(
            colorScheme = if (darkTheme) DarkScheme else LightScheme,
            typography = VexTypography,
            shapes = VexShapes,
            content = content,
        )
    }
}
