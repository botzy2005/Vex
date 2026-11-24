package com.vex.connect.ui.launch

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.vex.connect.core.designsystem.components.VexMark
import com.vex.connect.core.designsystem.theme.Inter
import kotlinx.coroutines.delay

/**
 * The launch screen: the V draws itself, then the wordmark fades in. It always uses the dark brand colours (the
 * Android splash before it is dark too), so the start of the app looks the same in every theme.
 */
@Composable
fun LaunchScreen(onFinished: () -> Unit) {
    val draw = remember { Animatable(0f) }
    val word = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        draw.animateTo(1f, tween(650, easing = FastOutSlowInEasing))
        word.animateTo(1f, tween(380))
        delay(350)
        onFinished()
    }
    Box(Modifier.fillMaxSize().background(Color(0xFF101114)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(18.dp)) {
            VexMark(size = 96.dp, progress = draw.value)
            Column(Modifier.alpha(word.value), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("VEX", fontFamily = Inter, fontWeight = FontWeight.Bold, fontSize = 34.sp, letterSpacing = 6.sp, color = Color(0xFFE7E9F0))
                Text(
                    "CONNECT",
                    fontFamily = Inter,
                    fontWeight = FontWeight.Medium,
                    fontSize = 12.sp,
                    letterSpacing = 5.sp,
                    color = Color(0xFF969BA8),
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
    }
}
