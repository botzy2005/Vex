package com.vex.connect.core.designsystem.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

/** Spacing scale (same 4-based rhythm as the desktop tokens). */
object Spacing {
    val xs = 4.dp
    val sm = 8.dp
    val md = 12.dp
    val lg = 16.dp
    val xl = 24.dp
    val xxl = 32.dp
    /** Side padding of every screen. */
    val screen = 20.dp
}

object Radii {
    val sm = 10.dp
    val md = 16.dp
    val lg = 24.dp
    val xl = 32.dp
}

internal val VexShapes = Shapes(
    extraSmall = RoundedCornerShape(Radii.sm),
    small = RoundedCornerShape(Radii.sm),
    medium = RoundedCornerShape(Radii.md),
    large = RoundedCornerShape(Radii.lg),
    extraLarge = RoundedCornerShape(Radii.xl),
)

/** Animation durations in milliseconds (desktop: micro 140, content 220, panel 340). */
object Motion {
    const val Micro = 140
    const val Content = 220
    const val Panel = 340
}
