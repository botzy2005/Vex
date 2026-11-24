# VEX Connect Android: build and install (Stage 1)

## What this build is

The Android app foundation: launch screen, first-run introduction, Home, Devices, Transfers and Settings screens, dark and
light themes, local settings and an (empty) local database. **It does not connect to anything.** It requests no permissions of its own
(the only entry in the APK is AndroidX's internal `DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION`, which is private to the app; CI fails if
the Internet permission ever appears in this stage).

## Where the APK comes from

The sandbox that develops VEX cannot download the Android SDK, so the app is built by GitHub Actions
(`.github/workflows/android.yml`). It runs when files under `android/` change, or by hand (Actions > android > Run workflow).
The Windows workflow is separate and untouched.

The workflow: installs JDK 17, uses a pinned Gradle (8.10.2), runs all unit tests, builds `app-debug.apk`, lists its permissions,
prints its signing certificate, and uploads the artifact **`vex-connect-debug-apk`** (APK, SHA-256, certificate, permissions).

## One-time setup: the fixed debug signing key

Android only lets an APK update an installed app when both are signed with the same key. CI machines create a new random debug
key every run, so without a fixed key each new build would first have to be uninstalled. Add four repository secrets
(GitHub > repository > Settings > Secrets and variables > Actions > New repository secret):

| Secret name | Value |
|---|---|
| `VEX_DEBUG_KEYSTORE_BASE64` | the debug keystore file, base64-encoded (one long line) |
| `VEX_DEBUG_KEYSTORE_PASSWORD` | the keystore password |
| `VEX_DEBUG_KEY_ALIAS` | `vexdebug` |
| `VEX_DEBUG_KEY_PASSWORD` | the key password (same as the keystore password for the supplied key) |

To make your own keystore instead (needs a JDK, for example the one inside Android Studio):

```
keytool -genkeypair -keystore vex-debug.jks -storetype PKCS12 -alias vexdebug -keyalg RSA -keysize 2048 -validity 10950
base64 -w0 vex-debug.jks        # Windows PowerShell: [Convert]::ToBase64String([IO.File]::ReadAllBytes("vex-debug.jks"))
```

This is a **debug** key. Never reuse it for a release build and never commit the keystore (`*.jks` is git-ignored). Release
signing will use a different key in Stage 11. If the secrets are missing the workflow still builds, using a temporary key, and
prints a warning.

## Download and install on the OnePlus Nord CE 3 Lite (OxygenOS 15)

1. On a computer or the phone, open the repository on GitHub > **Actions** > the latest green **android** run.
2. Under **Artifacts** download `vex-connect-debug-apk` (a zip; GitHub requires being signed in). Unzip it and get `app-debug.apk`.
   On the phone, a file manager can unzip it.
3. Copy `app-debug.apk` to the phone if you downloaded it on a computer (USB, or any transfer you like).
4. On the phone open the APK from the Files app. Android will say installing from this source is blocked: tap **Settings**,
   switch on **Allow from this source** for that app (Files or your browser), go back and tap **Install**.
5. If Play Protect warns that the app is unknown: choose **More details > Install anyway**. It is a debug build signed with your own key,
   not distributed through Google Play.
6. Open **VEX Connect**. You should see the launch animation, three introduction pages, then the Home screen saying
   "No PC paired yet".

Updating: install a newer APK over the old one (works only with the fixed key). If Android says the package conflicts with an
existing one, uninstall VEX Connect first (the app holds no data you need in Stage 1).

Optional check: the run summary shows the APK's SHA-256. On a computer: `sha256sum app-debug.apk` should match it.

## Build locally (optional, needs Android Studio)

Open the `android/` folder in Android Studio (Ladybug or newer) and press Run. Or, with the Android SDK and JDK 17 installed:

```
cd android
gradle wrapper --gradle-version 8.10.2     # once; the wrapper is not committed
./gradlew :app:assembleDebug
./gradlew :core:domain:test :core:data:testDebugUnitTest :app:testDebugUnitTest
```

Without the signing environment variables, Gradle uses the standard debug key in `~/.android/debug.keystore`.

## Tests in this stage

* `core/domain`: formatting, device-name cleaning, transfer filters (plain JVM).
* `core/data`: database entity mapping with unknown/damaged values, settings defaults and damaged-value handling.
* `app`: theme choice, launch/onboarding sequencing, transfer list states, every connection state's presentation, navigation routes.

Not covered yet (needs a device): visual appearance, animation smoothness, gestures, OxygenOS-specific behaviour, system theme switching.

## Design notes

* Colours and spacing mirror the VEX Desktop tokens (`src/styles/tokens.css`); typography is Inter (SIL Open Font License, see
  `android/core/designsystem/INTER_LICENSE.txt`).
* The default theme is Dark; Settings offers System, Dark and Light.
* Anything not built yet is shown as "Planned". Counters and lists read from the real local database, which is simply empty.
* Motion appears only for real progress: the launch animation and page changes, and a pulsing ring reserved for "connecting".
