package com.vex.connect

import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import com.vex.connect.ui.VexApp

class MainActivity : ComponentActivity() {
    // Links to paired PCs are kept only while the app is on screen (no background service in this stage).
    override fun onStart() {
        super.onStart()
        (application as VexConnectApp).container.links.setVisible(true)
    }

    override fun onStop() {
        (application as VexConnectApp).container.links.setVisible(false)
        super.onStop()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        installSplashScreen()
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
        )
        val container = (application as VexConnectApp).container
        setContent {
            VexApp(
                container = container,
                onSystemBars = { dark ->
                    // Light themes need dark status-bar icons; this follows the theme the user picked in Settings.
                    enableEdgeToEdge(
                        statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { dark },
                        navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { dark },
                    )
                },
            )
        }
    }
}
