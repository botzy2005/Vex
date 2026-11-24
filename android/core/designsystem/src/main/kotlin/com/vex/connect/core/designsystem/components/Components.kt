package com.vex.connect.core.designsystem.components

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.TextButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.vex.connect.core.designsystem.theme.Radii
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme

/** The standard rounded surface. Pass [onClick] to make it tappable. */
@Composable
fun VexCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val shape = RoundedCornerShape(Radii.lg)
    val border = BorderStroke(1.dp, VexTheme.extras.line)
    val inner: @Composable () -> Unit = {
        Column(Modifier.padding(Spacing.lg), content = content)
    }
    if (onClick != null) {
        Surface(
            onClick = onClick,
            modifier = modifier.fillMaxWidth(),
            shape = shape,
            color = MaterialTheme.colorScheme.surface,
            border = border,
            content = inner,
        )
    } else {
        Surface(
            modifier = modifier.fillMaxWidth(),
            shape = shape,
            color = MaterialTheme.colorScheme.surface,
            border = border,
            content = inner,
        )
    }
}

@Composable
fun ScreenHeader(title: String, modifier: Modifier = Modifier, subtitle: String? = null) {
    Column(modifier.padding(top = Spacing.lg, bottom = Spacing.sm)) {
        Text(title, style = MaterialTheme.typography.headlineLarge, color = MaterialTheme.colorScheme.onBackground)
        if (subtitle != null) {
            Text(
                subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = VexTheme.extras.textDim,
                modifier = Modifier.padding(top = Spacing.xs),
            )
        }
    }
}

@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = VexTheme.extras.textFaint,
        modifier = modifier.padding(top = Spacing.lg, bottom = Spacing.sm, start = Spacing.xs),
    )
}

enum class Tone { Neutral, Accent, Ok, Warn, Danger }

@Composable
fun toneColor(tone: Tone): Color = when (tone) {
    Tone.Neutral -> VexTheme.extras.textDim
    Tone.Accent -> MaterialTheme.colorScheme.primary
    Tone.Ok -> VexTheme.extras.ok
    Tone.Warn -> VexTheme.extras.warn
    Tone.Danger -> MaterialTheme.colorScheme.error
}

/** A small rounded label with a dot, for states ("Not connected", "Planned"). */
@Composable
fun StatusPill(text: String, tone: Tone, modifier: Modifier = Modifier, dot: Boolean = true) {
    val color = toneColor(tone)
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .background(color.copy(alpha = 0.14f))
            .padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (dot) Box(Modifier.size(6.dp).clip(CircleShape).background(color))
        Text(text, style = MaterialTheme.typography.labelMedium, color = color)
    }
}

/** The honest marker for something that is designed but not built yet. */
@Composable
fun PlannedTag(modifier: Modifier = Modifier, text: String = "Planned") =
    StatusPill(text = text, tone = Tone.Neutral, modifier = modifier, dot = false)

/**
 * A round status light. When [pulsing] is true the outer ring breathes slowly; it is only used for states that are
 * genuinely in progress (never while nothing is happening), so motion always means something.
 */
@Composable
fun StatusOrb(tone: Tone, icon: ImageVector, pulsing: Boolean, modifier: Modifier = Modifier) {
    val color = toneColor(tone)
    Box(modifier.size(72.dp), contentAlignment = Alignment.Center) {
        // The animation exists only while something is actually in progress, so a still screen costs nothing.
        if (pulsing) PulseRing(color)
        Box(
            Modifier.size(56.dp).clip(CircleShape).background(color.copy(alpha = 0.16f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(28.dp))
        }
    }
}

@Composable
private fun PulseRing(color: Color) {
    val transition = rememberInfiniteTransition(label = "orb")
    val ring by transition.animateFloat(
        initialValue = 1f,
        targetValue = 1.25f,
        animationSpec = infiniteRepeatable(tween(1600), RepeatMode.Reverse),
        label = "ring",
    )
    val ringAlpha by transition.animateFloat(
        initialValue = 0.28f,
        targetValue = 0.06f,
        animationSpec = infiniteRepeatable(tween(1600), RepeatMode.Reverse),
        label = "ringAlpha",
    )
    Box(Modifier.size(72.dp).scale(ring).alpha(ringAlpha).clip(CircleShape).background(color))
}

@Composable
fun EmptyState(
    icon: ImageVector,
    title: String,
    body: String,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit = {},
) {
    Column(
        modifier.fillMaxWidth().padding(vertical = Spacing.xl, horizontal = Spacing.lg),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        Box(
            Modifier.size(72.dp).clip(CircleShape).background(VexTheme.extras.elevated),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = VexTheme.extras.textDim, modifier = Modifier.size(32.dp))
        }
        Text(title, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
        Text(
            body,
            style = MaterialTheme.typography.bodyMedium,
            color = VexTheme.extras.textDim,
            textAlign = TextAlign.Center,
        )
        content()
    }
}

@Composable
fun VexButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    leading: ImageVector? = null,
) {
    Button(
        onClick = onClick,
        modifier = modifier.heightIn(min = 52.dp),
        enabled = enabled,
        shape = RoundedCornerShape(Radii.md),
        colors = ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
            disabledContainerColor = VexTheme.extras.elevated,
            disabledContentColor = VexTheme.extras.textFaint,
        ),
    ) {
        if (leading != null) {
            Icon(leading, contentDescription = null, modifier = Modifier.size(20.dp))
            Box(Modifier.size(8.dp))
        }
        Text(text, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
fun VexTextButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    TextButton(onClick = onClick, modifier = modifier, enabled = enabled) {
        Text(text, style = MaterialTheme.typography.labelLarge)
    }
}

/** One row of a settings list: title, optional subtitle, optional trailing content. */
@Composable
fun SettingRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    icon: ImageVector? = null,
    onClick: (() -> Unit)? = null,
    trailing: @Composable (RowScope.() -> Unit)? = null,
) {
    val rowModifier = modifier.fillMaxWidth().heightIn(min = 56.dp).let {
        if (onClick != null) it.clip(RoundedCornerShape(Radii.sm)).clickable(onClick = onClick) else it
    }
    Row(
        rowModifier.padding(vertical = Spacing.sm),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = VexTheme.extras.textDim, modifier = Modifier.size(22.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (subtitle != null) {
                Text(subtitle, style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
            }
        }
        if (trailing != null) trailing()
    }
}

@Composable
fun Divider(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(1.dp).background(VexTheme.extras.line))
}

/** A numbered step ("1  Open VEX on your PC"). */
@Composable
fun StepRow(number: Int, title: String, body: String, modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
        Box(
            Modifier.size(28.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary.copy(alpha = 0.16f)),
            contentAlignment = Alignment.Center,
        ) {
            Text(number.toString(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(body, style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
        }
    }
}
