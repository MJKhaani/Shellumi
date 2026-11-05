package org.shellumi.mobile

/**
 * JNI / C-ABI bridge interfacing with the portable Rust core (libshellumi_core.so).
 */
object ShellumiNativeCore {
    init {
        try {
            System.loadLibrary("shellumi_core")
        } catch (e: UnsatisfiedLinkError) {
            System.err.println("Warning: libshellumi_core.so not found on standard java library path: ${e.message}")
        }
    }

    const val MODIFIER_CTRL: Int = 0x01
    const val MODIFIER_ALT: Int = 0x02
    const val MODIFIER_SHIFT: Int = 0x04

    @JvmStatic
    external fun coreVersion(): String

    @JvmStatic
    external fun mobilePlatformSupported(platform: String): Boolean

    @JvmStatic
    external fun parseDestination(target: String): String

    @JvmStatic
    external fun generateKeypair(comment: String?): String

    @JvmStatic
    external fun translateVirtualKey(keyName: String, modifiers: Int): String

    @JvmStatic
    external fun lifecycleTransition(state: String, event: String, param: Long, epoch: Long): String

    @JvmStatic
    external fun getCredentialSpec(
        platform: String,
        service: String,
        account: String,
        biometric: Boolean
    ): String
}
