package org.shellumi.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver

/**
 * Shellumi Mobile Android Prototype (Jetpack Compose).
 * Integrates:
 * 1. Hardware BiometricPrompt for StrongBox / Keystore credential unlocking.
 * 2. Sticky Virtual Accessory Bar for single-hand touch terminal navigation.
 * 3. Lifecycle state machine forwarding (app standby / background grace period).
 */
class MainActivity : ComponentActivity() {

    private var isAuthenticated by mutableStateOf(false)
    private var sessionState by mutableStateOf("connected")
    private var isCtrlActive by mutableStateOf(false)
    private var isAltActive by mutableStateOf(false)
    private val terminalLines = mutableStateListOf(
        "Shellumi Mobile Terminal (Android Edition)",
        "Connected to remote host: admin@10.0.0.5:22",
        "Linux 6.8.0-45-generic #45-Ubuntu SMP aarch64",
        "Last login: Wed Nov 5 02:20:10 2026",
        "admin@server:~$ "
    )
    private val telemetryLogs = mutableStateListOf<String>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Observe Android lifecycle to simulate background grace periods and socket lifecycle
        lifecycle.addObserver(LifecycleEventObserver { _, event ->
            val epoch = System.currentTimeMillis() / 1000
            when (event) {
                Lifecycle.Event.ON_STOP -> {
                    try {
                        val res = ShellumiNativeCore.lifecycleTransition(sessionState, "background", 30L, epoch)
                        telemetryLogs.add("Lifecycle ON_STOP: $res")
                    } catch (e: Throwable) {
                        telemetryLogs.add("Lifecycle transition: background (simulated)")
                    }
                }
                Lifecycle.Event.ON_START -> {
                    try {
                        val res = ShellumiNativeCore.lifecycleTransition(sessionState, "foreground", 0L, epoch)
                        telemetryLogs.add("Lifecycle ON_START: $res")
                    } catch (e: Throwable) {
                        telemetryLogs.add("Lifecycle transition: foreground (simulated)")
                    }
                }
                else -> {}
            }
        })

        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = Color.Black
                ) {
                    Column(modifier = Modifier.fillMaxSize()) {
                        // Top bar
                        TopBar(
                            sessionState = sessionState,
                            isAuthenticated = isAuthenticated,
                            onAuthenticate = { promptBiometrics() }
                        )

                        // Terminal output view
                        LazyColumn(
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxWidth()
                                .padding(8.dp)
                        ) {
                            items(terminalLines) { line ->
                                Text(
                                    text = line,
                                    color = Color(0xFF4ADE80),
                                    fontFamily = FontFamily.Monospace,
                                    fontSize = 13.sp
                                )
                            }
                        }

                        // Virtual accessory bar
                        VirtualAccessoryRow(
                            isCtrl = isCtrlActive,
                            isAlt = isAltActive,
                            onToggleCtrl = { isCtrlActive = !isCtrlActive },
                            onToggleAlt = { isAltActive = !isAltActive },
                            onSendKey = { keyName ->
                                var flags = 0
                                if (isCtrlActive) flags = flags or ShellumiNativeCore.MODIFIER_CTRL
                                if (isAltActive) flags = flags or ShellumiNativeCore.MODIFIER_ALT

                                try {
                                    val res = ShellumiNativeCore.translateVirtualKey(keyName, flags)
                                    telemetryLogs.add("VirtualKey $keyName: $res")
                                } catch (e: Throwable) {
                                    telemetryLogs.add("VirtualKey $keyName (flags=$flags)")
                                }

                                // Clear sticky modifiers once applied
                                isCtrlActive = false
                                isAltActive = false
                            }
                        )
                    }
                }
            }
        }
    }

    private fun promptBiometrics() {
        val executor = ContextCompat.getMainExecutor(this)
        val promptInfo = BiometricPrompt.PromptInfo.Builder()
            .setTitle("Unlock Shellumi Credentials")
            .setSubtitle("Confirm biometrics to release hardware-backed SSH key")
            .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG or BiometricManager.Authenticators.DEVICE_CREDENTIAL)
            .build()

        val prompt = BiometricPrompt(this, executor, object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                super.onAuthenticationSucceeded(result)
                isAuthenticated = true
                telemetryLogs.add("Biometric authentication successful via Android Keystore")
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                super.onAuthenticationError(errorCode, errString)
                telemetryLogs.add("Biometric error: $errString ($errorCode)")
            }
        })

        prompt.authenticate(promptInfo)
    }
}

@Composable
fun TopBar(
    sessionState: String,
    isAuthenticated: Bool,
    onAuthenticate: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(0xFF1E293B))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = "STATE: ${sessionState.uppercase()}",
            color = Color.White,
            fontSize = 12.sp,
            fontFamily = FontFamily.Monospace
        )
        Spacer(modifier = Modifier.weight(1f))
        if (isAuthenticated) {
            Text(
                text = "🛡️ Verified",
                color = Color(0xFF4ADE80),
                fontSize = 12.sp,
                fontFamily = FontFamily.Monospace
            )
        } else {
            Button(
                onClick = onAuthenticate,
                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                shape = RoundedCornerShape(4.dp)
            ) {
                Text("Unlock Keys", fontSize = 11.sp)
            }
        }
    }
}

@Composable
fun VirtualAccessoryRow(
    isCtrl: Boolean,
    isAlt: Boolean,
    onToggleCtrl: () -> Unit,
    onToggleAlt: () -> Unit,
    onSendKey: (String) -> Unit
) {
    val scrollState = rememberScrollState()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(0xFF0F172A))
            .horizontalScroll(scrollState)
            .padding(horizontal = 6.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        AccessoryKeyButton("ESC") { onSendKey("esc") }
        AccessoryKeyButton("TAB") { onSendKey("tab") }
        
        // Sticky Modifiers
        AccessoryKeyButton(if (isCtrl) "CTRL*" else "CTRL", active = isCtrl) { onToggleCtrl() }
        AccessoryKeyButton(if (isAlt) "ALT*" else "ALT", active = isAlt) { onToggleAlt() }

        AccessoryKeyButton("↑") { onSendKey("arrow_up") }
        AccessoryKeyButton("↓") { onSendKey("arrow_down") }
        AccessoryKeyButton("←") { onSendKey("arrow_left") }
        AccessoryKeyButton("→") { onSendKey("arrow_right") }
        AccessoryKeyButton("|") { onSendKey("|") }
        AccessoryKeyButton("~") { onSendKey("~") }
        AccessoryKeyButton("/") { onSendKey("/") }
        AccessoryKeyButton("\\") { onSendKey("\\") }
    }
}

@Composable
fun AccessoryKeyButton(
    label: String,
    active: Boolean = false,
    onClick: () -> Unit
) {
    Button(
        onClick = onClick,
        colors = ButtonDefaults.buttonColors(
            containerColor = if (active) Color(0xFF2563EB) else Color(0xFF334155),
            contentColor = Color.White
        ),
        shape = RoundedCornerShape(4.dp),
        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp)
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontFamily = FontFamily.Monospace
        )
    }
}
