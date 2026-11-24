# VEX Connect for Android

Kotlin + Jetpack Compose + Material 3. Companion to VEX Desktop. **Stage 1: app foundation only.** There is no
networking, pairing or file transfer in this build, and it does not request the Internet permission.

Build, install and signing instructions: [`docs/VEX_CONNECT_ANDROID.md`](../docs/VEX_CONNECT_ANDROID.md).

## Layout

```
android/
  app/                    UI (Compose screens, navigation, ViewModels), Application, manifest, launcher icon
  core/designsystem/      Theme, design tokens, Inter font, reusable components (no app logic)
  core/domain/            Pure Kotlin models, repository interfaces, formatting/validation rules (JVM tests)
  core/data/              Room database (paired devices, transfer history), DataStore settings, mappers
  gradle/libs.versions.toml   every dependency version, pinned
```

Layers depend downward only: `app` -> `domain` + `data` + `designsystem`; `data` -> `domain`. The domain module has no Android
dependency, so its tests run on the plain JVM.
