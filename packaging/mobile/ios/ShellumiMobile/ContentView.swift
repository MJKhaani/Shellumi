//
// ContentView.swift
// Shellumi Mobile iOS Prototype
//
// Demonstrates native SwiftUI UI consuming Shellumi portable Rust core via C-ABI / UniFFI:
// 1. Hardware biometric authentication (Face ID / Touch ID via LocalAuthentication).
// 2. Sticky virtual accessory bar (Esc, Tab, Ctrl, Alt, Arrow keys).
// 3. iOS ScenePhase lifecycle bridging into Rust MobileLifecycleManager (~30s grace period).
//

import SwiftUI
import LocalAuthentication

// MARK: - C-ABI Bridge Declarations

@_silgen_name("shellumi_core_version")
func shellumi_core_version() -> UnsafePointer<CChar>

@_silgen_name("shellumi_mobile_platform_supported")
func shellumi_mobile_platform_supported(_ platform: UnsafePointer<CChar>) -> Bool

@_silgen_name("shellumi_parse_destination_c")
func shellumi_parse_destination_c(_ target: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>

@_silgen_name("shellumi_generate_keypair_c")
func shellumi_generate_keypair_c(_ comment: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>

@_silgen_name("shellumi_translate_virtual_key_c")
func shellumi_translate_virtual_key_c(_ key: UnsafePointer<CChar>, _ modifiers: UInt32) -> UnsafeMutablePointer<CChar>

@_silgen_name("shellumi_lifecycle_transition_c")
func shellumi_lifecycle_transition_c(_ state: UnsafePointer<CChar>, _ event: UnsafePointer<CChar>, _ param: UInt64, _ epoch: UInt64) -> UnsafeMutablePointer<CChar>

@_silgen_name("shellumi_string_free")
func shellumi_string_free(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - Swift Wrapper

final class ShellumiCoreWrapper: ObservableObject {
    @Published var sessionState: String = "connected"
    @Published var isActive: Bool = true
    @Published var terminalLines: [String] = [
        "Shellumi Mobile Terminal (v\(String(cString: shellumi_core_version())))",
        "Connected to remote host: dev@192.168.1.100:22",
        "Linux 6.8.0-45-generic #45-Ubuntu SMP x86_64",
        "Last login: Wed Nov 5 02:15:30 2026 from 10.0.0.4",
        "dev@ubuntu-server:~$ "
    ]
    @Published var ctrlActive: Bool = false
    @Published var altActive: Bool = false
    @Published var telemetryLog: [String] = []
    @Published var isAuthenticated: Bool = false

    func sendKey(_ keyName: String) {
        var flags: UInt32 = 0
        if ctrlActive { flags |= 0x01 }
        if altActive { flags |= 0x02 }

        keyName.withCString { cKey in
            let resPtr = shellumi_translate_virtual_key_c(cKey, flags)
            defer { shellumi_string_free(resPtr) }

            let jsonStr = String(cString: resPtr)
            telemetryLog.append("VirtualKey '\(keyName)': \(jsonStr)")
            if telemetryLog.count > 10 { telemetryLog.removeFirst() }

            // Auto-clear sticky modifiers once applied
            if ctrlActive || altActive {
                ctrlActive = false
                altActive = false
            }
        }
    }

    func authenticateBiometrics(completion: @escaping (Bool) -> Void) {
        let context = LAContext()
        var error: NSError?

        if context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) {
            let reason = "Authenticate with Face ID / Touch ID to access SSH private keys"
            context.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, localizedReason: reason) { success, _ in
                DispatchQueue.main.async {
                    self.isAuthenticated = success
                    completion(success)
                }
            }
        } else {
            // Simulator or hardware without biometrics: fallback
            self.isAuthenticated = true
            completion(true)
        }
    }

    func transitionLifecycle(event: String, param: UInt64 = 0) {
        let epoch = UInt64(Date().timeIntervalSince1970)
        sessionState.withCString { cState in
            event.withCString { cEvent in
                let resPtr = shellumi_lifecycle_transition_c(cState, cEvent, param, epoch)
                defer { shellumi_string_free(resPtr) }
                telemetryLog.append("Lifecycle '\(event)': \(String(cString: resPtr))")
            }
        }
    }
}

// MARK: - SwiftUI Views

struct ContentView: View {
    @StateObject private var core = ShellumiCoreWrapper()
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Header status banner
                HStack {
                    Circle()
                        .fill(core.isActive ? Color.green : Color.orange)
                        .frame(width: 10, height: 10)
                    Text("State: \(core.sessionState.uppercased())")
                        .font(.caption.bold())
                    Spacer()
                    if core.isAuthenticated {
                        Label("Biometrics Verified", systemImage: "checkmark.shield.fill")
                            .font(.caption2)
                            .foregroundColor(.green)
                    } else {
                        Button("Unlock Keys") {
                            core.authenticateBiometrics { _ in }
                        }
                        .font(.caption2.bold())
                        .buttonStyle(.borderedProminent)
                        .tint(.blue)
                    }
                }
                .padding(.horizontal)
                .padding(.vertical, 8)
                .background(Color(uiColor: .secondarySystemBackground))

                // Terminal viewport
                ScrollView {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(core.terminalLines, id: \.self) { line in
                            Text(line)
                                .font(.system(.footnote, design: .monospaced))
                                .foregroundColor(.green)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                .background(Color.black)

                // Virtual Accessory Bar
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        Button("ESC") { core.sendKey("esc") }.buttonStyle(AccessoryButtonStyle())
                        Button("TAB") { core.sendKey("tab") }.buttonStyle(AccessoryButtonStyle())
                        
                        // Sticky modifier toggles
                        Button(core.ctrlActive ? "CTRL*" : "CTRL") {
                            core.ctrlActive.toggle()
                        }
                        .buttonStyle(AccessoryButtonStyle(active: core.ctrlActive))

                        Button(core.altActive ? "ALT*" : "ALT") {
                            core.altActive.toggle()
                        }
                        .buttonStyle(AccessoryButtonStyle(active: core.altActive))

                        Button("↑") { core.sendKey("arrow_up") }.buttonStyle(AccessoryButtonStyle())
                        Button("↓") { core.sendKey("arrow_down") }.buttonStyle(AccessoryButtonStyle())
                        Button("←") { core.sendKey("arrow_left") }.buttonStyle(AccessoryButtonStyle())
                        Button("→") { core.sendKey("arrow_right") }.buttonStyle(AccessoryButtonStyle())
                        Button("|") { core.sendKey("|") }.buttonStyle(AccessoryButtonStyle())
                        Button("~") { core.sendKey("~") }.buttonStyle(AccessoryButtonStyle())
                        Button("/") { core.sendKey("/") }.buttonStyle(AccessoryButtonStyle())
                        Button("\\") { core.sendKey("\\") }.buttonStyle(AccessoryButtonStyle())
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                }
                .background(Color(uiColor: .systemGray6))
            }
            .navigationTitle("Shellumi Mobile")
            .navigationBarTitleDisplayMode(.inline)
            .onChange(of: scenePhase) { newPhase in
                switch newPhase {
                case .background:
                    // iOS app moved to background: start ~30s grace timer in Rust core
                    core.transitionLifecycle(event: "background", param: 30)
                case .active:
                    // App foregrounded
                    core.transitionLifecycle(event: "foreground")
                default:
                    break
                }
            }
        }
    }
}

struct AccessoryButtonStyle: ButtonStyle {
    var active: Bool = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(.caption2, design: .monospaced).bold())
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(active ? Color.blue : (configuration.isPressed ? Color.gray : Color(uiColor: .systemGray5)))
            .foregroundColor(active ? .white : .primary)
            .cornerRadius(6)
    }
}
