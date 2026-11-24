package com.vex.connect.core.designsystem.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.vex.connect.core.designsystem.theme.VexTheme

/**
 * The VEX "V" in the brand gradient. [progress] draws it stroke by stroke (0 = nothing, 1 = complete), which the
 * launch screen uses for its entrance. The same geometry is used by the launcher icon (res/drawable/ic_launcher_foreground.xml).
 */
@Composable
fun VexMark(modifier: Modifier = Modifier, size: Dp = 56.dp, progress: Float = 1f) {
    val brush = VexTheme.extras.brand
    Canvas(modifier.size(size)) {
        val w = this.size.width
        val h = this.size.height
        val full = Path().apply {
            moveTo(w * 0.17f, h * 0.22f)
            lineTo(w * 0.50f, h * 0.80f)
            lineTo(w * 0.83f, h * 0.22f)
        }
        val shown = if (progress >= 1f) {
            full
        } else {
            val measure = PathMeasure().apply { setPath(full, false) }
            Path().also { measure.getSegment(0f, measure.length * progress.coerceIn(0f, 1f), it, true) }
        }
        drawPath(
            path = shown,
            brush = brush,
            style = Stroke(width = w * 0.17f, cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
}
