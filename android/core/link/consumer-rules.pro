# The generated UniFFI bindings are called through JNA by reflection.
-keep class com.sun.jna.** { *; }
-keep class uniffi.vex_link_ffi.** { *; }
-dontwarn java.awt.*
