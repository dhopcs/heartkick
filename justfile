# Override any of these via environment variables or a .env file:
#   ANDROID_HOME, NDK_HOME, ANDROID_BUILD_TOOLS, HK_KEYSTORE, HK_KS_PASS
android_home      := env("ANDROID_HOME", env("ANDROID_SDK_ROOT", "$HOME/Android/Sdk"))
ndk_version       := env("NDK_VERSION", "29.0.13846066")
build_tools       := env("ANDROID_BUILD_TOOLS", "35.0.0")
ndk_home          := env("NDK_HOME", android_home / "ndk" / ndk_version)
bt                := android_home / "build-tools" / build_tools
ks                := env("HK_KEYSTORE", "src-tauri/gen/android/heartkick.jks")
ks_pass           := env("HK_KS_PASS", "heartkick")
apk_unsigned      := "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
apk_aligned       := "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-aligned.apk"
apk_signed        := "src-tauri/gen/android/app/build/outputs/apk/universal/release/heartkick-signed.apk"

android_env := "ANDROID_HOME=" + android_home + " ANDROID_SDK_ROOT=" + android_home + " NDK_HOME=" + ndk_home

# List available recipes
default:
    @just --list

# Run desktop dev server
dev:
    pnpm tauri dev

# Run Android dev on connected USB device
dev-android device="":
    #!/usr/bin/env bash
    set -euo pipefail
    export ANDROID_HOME="{{ android_home }}" ANDROID_SDK_ROOT="{{ android_home }}" NDK_HOME="{{ ndk_home }}"
    dev="{{ device }}"
    if [[ -z "$dev" ]]; then
        dev=$(adb devices | awk '/\tdevice$/{print $1; exit}')
    fi
    pnpm tauri android dev --device "$dev"

# Build desktop release
build:
    pnpm tauri build

# Generate a local signing keystore (only needed once)
make-keystore:
    keytool -genkey -v -keystore "{{ ks }}" \
        -alias heartkick -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass "{{ ks_pass }}" -keypass "{{ ks_pass }}" \
        -dname "CN=heartkick, O=heartkick, C=US"

# Build Android release APK
build-android:
    {{ android_env }} pnpm tauri android build

sign-apk:
    {{ bt }}/zipalign -f -p 4 "{{ apk_unsigned }}" "{{ apk_aligned }}"
    {{ bt }}/apksigner sign \
        --ks "{{ ks }}" --ks-pass pass:{{ ks_pass }} \
        --ks-key-alias heartkick --key-pass pass:{{ ks_pass }} \
        --out "{{ apk_signed }}" "{{ apk_aligned }}"

# Build, sign, and install APK on connected device
local-install: build-android sign-apk
    adb install -r "{{ apk_signed }}"

# Lint TypeScript/JS with oxlint
lint:
    pnpm oxlint -c .oxlintrc.json src/

# Lint and auto-fix
lint-fix:
    pnpm oxlint -c .oxlintrc.json --fix src/

# Format TypeScript/JS with oxfmt
fmt:
    pnpm oxfmt src/

# Check formatting without writing
fmt-check:
    pnpm oxfmt --check src/

# Lint + typecheck Rust
lint-rust:
    cd src-tauri && cargo clippy --all-targets -- -D warnings

# Format Rust
fmt-rust:
    cd src-tauri && cargo fmt

# Run all lints and format checks
check: fmt-check lint lint-rust

# Run all formatters and fixers
fix: fmt lint-fix fmt-rust
