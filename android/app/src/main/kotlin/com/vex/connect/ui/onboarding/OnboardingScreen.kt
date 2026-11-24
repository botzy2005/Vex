package com.vex.connect.ui.onboarding

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.Shield
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.vex.connect.core.designsystem.components.PlannedTag
import com.vex.connect.core.designsystem.components.VexButton
import com.vex.connect.core.designsystem.components.VexMark
import com.vex.connect.core.designsystem.components.VexTextButton
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import kotlinx.coroutines.launch

private data class Page(val icon: ImageVector?, val title: String, val body: String, val note: String)

private val pages = listOf(
    Page(
        icon = null,
        title = "Your PC and phone, closer",
        body = "VEX Connect is the Android companion to VEX Desktop. Its goal is to move files, share the clipboard and show phone notifications on your PC.",
        note = "This first version is the app foundation. Connecting comes in the next stages.",
    ),
    Page(
        icon = Icons.Rounded.Lock,
        title = "Private by design",
        body = "The plan is a short-lived QR code to pair, encrypted connections, and no account. On your own Wi-Fi, nothing has to leave your home network.",
        note = "None of this is switched on yet. This version has no network access at all.",
    ),
    Page(
        icon = Icons.Rounded.Shield,
        title = "You stay in control",
        body = "Incoming files will always need your approval. You will be able to rename, disconnect or remove any device, and choose which features are on.",
        note = "These controls arrive together with the features they protect.",
    ),
)

@Composable
fun OnboardingScreen(onDone: () -> Unit) {
    val pager = rememberPagerState(pageCount = { pages.size })
    val scope = rememberCoroutineScope()
    val last = pager.currentPage == pages.lastIndex

    Column(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding().padding(horizontal = Spacing.screen),
    ) {
        Row(Modifier.fillMaxWidth().height(52.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f))
            if (!last) VexTextButton("Skip", onClick = onDone)
        }
        HorizontalPager(state = pager, modifier = Modifier.weight(1f)) { index ->
            PageContent(pages[index])
        }
        Row(
            Modifier.fillMaxWidth().padding(vertical = Spacing.lg),
            horizontalArrangement = Arrangement.Center,
        ) {
            repeat(pages.size) { i ->
                val selected = pager.currentPage == i
                val width = animateDpAsState(if (selected) 24.dp else 8.dp, label = "dotWidth")
                val color = animateColorAsState(
                    if (selected) MaterialTheme.colorScheme.primary else VexTheme.extras.line,
                    label = "dotColor",
                )
                Box(
                    Modifier.padding(horizontal = 3.dp).height(8.dp).width(width.value).clip(CircleShape).background(color.value),
                )
            }
        }
        VexButton(
            text = if (last) "Get started" else "Next",
            onClick = { if (last) onDone() else scope.launch { pager.animateScrollToPage(pager.currentPage + 1) } },
            modifier = Modifier.fillMaxWidth(),
        )
        Box(Modifier.height(Spacing.lg))
    }
}

@Composable
private fun PageContent(page: Page) {
    Column(
        Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            Modifier.size(136.dp).clip(RoundedCornerShape(40.dp)).background(VexTheme.extras.elevated),
            contentAlignment = Alignment.Center,
        ) {
            if (page.icon == null) {
                VexMark(size = 72.dp)
            } else {
                Icon(page.icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(60.dp))
            }
        }
        Box(Modifier.height(Spacing.xl))
        Text(page.title, style = MaterialTheme.typography.headlineMedium, textAlign = TextAlign.Center)
        Box(Modifier.height(Spacing.md))
        Text(page.body, style = MaterialTheme.typography.bodyLarge, color = VexTheme.extras.textDim, textAlign = TextAlign.Center)
        Box(Modifier.height(Spacing.lg))
        PlannedTag(text = "Not built yet")
        Box(Modifier.height(Spacing.sm))
        Text(page.note, style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textFaint, textAlign = TextAlign.Center)
    }
}
