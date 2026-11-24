package com.vex.connect.core.designsystem.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color

/** Colours that Material 3's scheme has no slot for. Values mirror the VEX Desktop tokens (src/styles/tokens.css). */
@Immutable
data class VexExtras(
    val elevated: Color,
    val line: Color,
    val textDim: Color,
    val textFaint: Color,
    val ok: Color,
    val warn: Color,
    val brandStart: Color,
    val brandEnd: Color,
) {
    val brand: Brush get() = Brush.linearGradient(listOf(brandStart, brandEnd))
}

internal val DarkExtras = VexExtras(
    elevated = Color(0xFF252832),
    line = Color(0x14FFFFFF),
    textDim = Color(0xFF969BA8),
    textFaint = Color(0xFF666B78),
    ok = Color(0xFF4CC38A),
    warn = Color(0xFFE5A94B),
    brandStart = Color(0xFF6FA8FF),
    brandEnd = Color(0xFFB07CFF),
)

internal val LightExtras = VexExtras(
    elevated = Color(0xFFECEEF3),
    line = Color(0x1A000000),
    textDim = Color(0xFF5B6070),
    textFaint = Color(0xFF8A8F9D),
    ok = Color(0xFF1F8A5B),
    warn = Color(0xFFB7791F),
    brandStart = Color(0xFF4F86E8),
    brandEnd = Color(0xFF8B5CF0),
)

internal val DarkScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFFA5B4FC),
    onPrimary = Color(0xFF101114),
    primaryContainer = Color(0xFF2C3150),
    onPrimaryContainer = Color(0xFFDDE1FF),
    secondary = Color(0xFF969BA8),
    onSecondary = Color(0xFF101114),
    background = Color(0xFF101114),
    onBackground = Color(0xFFE7E9F0),
    surface = Color(0xFF1B1D23),
    onSurface = Color(0xFFE7E9F0),
    surfaceVariant = Color(0xFF252832),
    onSurfaceVariant = Color(0xFF969BA8),
    surfaceContainerLowest = Color(0xFF101114),
    surfaceContainerLow = Color(0xFF16181D),
    surfaceContainer = Color(0xFF1B1D23),
    surfaceContainerHigh = Color(0xFF21242C),
    surfaceContainerHighest = Color(0xFF252832),
    outline = Color(0xFF666B78),
    outlineVariant = Color(0x14FFFFFF),
    error = Color(0xFFFF7B72),
    onError = Color(0xFF101114),
)

internal val LightScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF5B6CD6),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFE0E4FF),
    onPrimaryContainer = Color(0xFF1B2160),
    secondary = Color(0xFF5B6070),
    onSecondary = Color(0xFFFFFFFF),
    background = Color(0xFFF3F4F7),
    onBackground = Color(0xFF17181D),
    surface = Color(0xFFFFFFFF),
    onSurface = Color(0xFF17181D),
    surfaceVariant = Color(0xFFECEEF3),
    onSurfaceVariant = Color(0xFF5B6070),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF8F9FB),
    surfaceContainer = Color(0xFFFFFFFF),
    surfaceContainerHigh = Color(0xFFF1F2F6),
    surfaceContainerHighest = Color(0xFFECEEF3),
    outline = Color(0xFF8A8F9D),
    outlineVariant = Color(0x1A000000),
    error = Color(0xFFC9362D),
    onError = Color(0xFFFFFFFF),
)
