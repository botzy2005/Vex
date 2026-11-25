package com.vex.connect.ui.pair

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.vex.connect.core.designsystem.components.ScreenHeader
import com.vex.connect.core.designsystem.components.VexButton
import com.vex.connect.core.designsystem.components.VexCard
import com.vex.connect.core.designsystem.components.VexTextButton
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.core.domain.model.PairingUi
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.PairViewModel

@Composable
fun PairScreen(factory: AppViewModelFactory, contentPadding: PaddingValues, onClose: () -> Unit) {
    val vm: PairViewModel = viewModel(factory = factory)
    val state by vm.state.collectAsStateWithLifecycle()
    BackHandler { onClose() }

    Column(
        Modifier
            .fillMaxSize()
            .padding(top = contentPadding.calculateTopPadding(), bottom = contentPadding.calculateBottomPadding())
            .padding(horizontal = Spacing.screen)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        ScreenHeader("Pair a PC", subtitle = "Both devices must be on the same Wi-Fi")
        when (val s = state) {
            PairingUi.Idle -> ScanStep(onCode = vm::start, onCancel = onClose)
            PairingUi.Connecting -> Busy("Connecting to your PC…", onCancel = { vm.cancel(); onClose() })
            is PairingUi.Confirm -> Compare(s.code, s.pcName, waiting = false, onAnswer = vm::answer)
            is PairingUi.WaitingForPc -> Compare(s.code, s.pcName, waiting = true, onAnswer = vm::answer)
            is PairingUi.Done -> Outcome(
                ok = true,
                title = "Paired with ${s.pcName}",
                body = "This phone and ${s.pcName} now trust each other. You can rename or remove it from the Devices tab.",
                primary = "Done" to onClose,
            )
            is PairingUi.Declined -> Outcome(
                ok = false,
                title = "Pairing cancelled",
                body = if (s.byPhone) "You said the codes did not match, so nothing was paired." else "The PC declined, so nothing was paired.",
                primary = "Try again" to vm::cancel,
                secondary = "Close" to onClose,
            )
            is PairingUi.Failed -> Outcome(
                ok = false,
                title = "Pairing did not finish",
                body = s.message,
                primary = "Try again" to vm::cancel,
                secondary = "Close" to onClose,
            )
        }
    }
}

@Composable
private fun ScanStep(onCode: (String) -> Unit, onCancel: () -> Unit) {
    val context = LocalContext.current
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    var scanning by remember { mutableStateOf(granted) }
    var pasting by remember { mutableStateOf(false) }
    var denied by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it
        scanning = it
        denied = !it
    }

    if (pasting) {
        PasteStep(onCode = onCode, onBack = { pasting = false })
        return
    }

    VexCard {
        Text("On your PC, open VEX Connect from the VEX tray menu, turn it on and choose Pair a new phone.", style = MaterialTheme.typography.bodyMedium)
    }
    if (scanning && granted) {
        Box(Modifier.fillMaxWidth().height(320.dp).clip(RoundedCornerShape(20.dp))) {
            QrScanner(Modifier.fillMaxSize()) { text -> if (text.startsWith("vexlink:")) onCode(text) }
        }
        Text(
            "Point the camera at the QR code. The camera is used only to read this code; nothing is recorded or sent.",
            style = MaterialTheme.typography.bodySmall,
            color = VexTheme.extras.textDim,
        )
    } else {
        VexCard {
            Icon(Icons.Rounded.QrCodeScanner, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            Text("Camera access", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = Spacing.sm))
            Text(
                if (denied) "Camera access was not allowed. You can still pair by pasting the code that VEX shows on your PC."
                else "VEX Connect needs the camera only to scan the pairing QR code. Images are analysed on this phone and never stored or sent.",
                style = MaterialTheme.typography.bodyMedium,
                color = VexTheme.extras.textDim,
                modifier = Modifier.padding(top = Spacing.xs),
            )
            if (!denied) VexButton("Allow camera and scan", { ask.launch(Manifest.permission.CAMERA) }, Modifier.fillMaxWidth().padding(top = Spacing.md))
        }
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        VexTextButton("Paste the code instead", { pasting = true })
        VexTextButton("Cancel", onCancel)
    }
}

@Composable
private fun PasteStep(onCode: (String) -> Unit, onBack: () -> Unit) {
    var text by remember { mutableStateOf("") }
    VexCard {
        Text("On your PC, copy the text shown under the QR code, then paste it here.", style = MaterialTheme.typography.bodyMedium)
        OutlinedTextField(
            value = text,
            onValueChange = { text = it.take(2000) },
            label = { Text("Pairing code") },
            modifier = Modifier.fillMaxWidth().padding(top = Spacing.md),
            minLines = 3,
        )
    }
    VexButton("Pair", { onCode(text.trim()) }, Modifier.fillMaxWidth(), enabled = text.trim().startsWith("vexlink:"))
    VexTextButton("Back to scanning", onBack)
}

@Composable
private fun Busy(message: String, onCancel: () -> Unit) {
    VexCard {
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Spacing.md)) {
            CircularProgressIndicator()
            Text(message, style = MaterialTheme.typography.bodyMedium)
        }
    }
    VexTextButton("Cancel", onCancel)
}

@Composable
private fun Compare(code: String, pcName: String, waiting: Boolean, onAnswer: (Boolean) -> Unit) {
    VexCard {
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            Text("Does $pcName show this code?", style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
            Text(
                code,
                fontSize = 44.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(vertical = Spacing.lg),
            )
            Text(
                if (waiting) "Waiting for you to confirm on the PC…"
                else "Only continue if the code on the PC is exactly the same. If it is different, someone else may be trying to pair.",
                style = MaterialTheme.typography.bodySmall,
                color = VexTheme.extras.textDim,
                textAlign = TextAlign.Center,
            )
        }
    }
    if (!waiting) VexButton("Yes, it matches", { onAnswer(true) }, Modifier.fillMaxWidth())
    VexTextButton(if (waiting) "Cancel" else "No, cancel", { onAnswer(false) }, Modifier.fillMaxWidth())
}

@Composable
private fun Outcome(ok: Boolean, title: String, body: String, primary: Pair<String, () -> Unit>, secondary: Pair<String, () -> Unit>? = null) {
    VexCard {
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(
                if (ok) Icons.Rounded.CheckCircle else Icons.Rounded.ErrorOutline,
                contentDescription = null,
                tint = if (ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
            )
            Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center, modifier = Modifier.padding(top = Spacing.md))
            Text(body, style = MaterialTheme.typography.bodyMedium, color = VexTheme.extras.textDim, textAlign = TextAlign.Center, modifier = Modifier.padding(top = Spacing.sm))
        }
    }
    VexButton(primary.first, primary.second, Modifier.fillMaxWidth())
    secondary?.let { VexTextButton(it.first, it.second, Modifier.fillMaxWidth()) }
}
