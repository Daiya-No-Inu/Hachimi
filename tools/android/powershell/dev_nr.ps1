#Requires -Version 5.0
$ErrorActionPreference = "Stop"

# --- CONFIG ---
$APK_EXTRACT_DIR = "$env:TEMP\hachimi-apk-extract"
$TMP_BASE_APK = "$env:TEMP\hachimi-base.apk"
$TMP_CONFIG_APK = "$env:TEMP\hachimi-config.apk"


$APK_ARM64_LIB_DIR = Join-Path $APK_EXTRACT_DIR "lib\arm64-v8a"
$APK_ARM_LIB_DIR   = Join-Path $APK_EXTRACT_DIR "lib\armeabi-v7a"
$APK_X86_LIB_DIR   = Join-Path $APK_EXTRACT_DIR "lib\x86"
$APK_X86_64_LIB_DIR= Join-Path $APK_EXTRACT_DIR "lib\x86_64"

if (-not $env:PACKAGE_NAME) { $env:PACKAGE_NAME = "jp.co.cygames.umamusume" }
if (-not $env:ACTIVITY_NAME) { $env:ACTIVITY_NAME = "jp.co.cygames.umamusume_activity.UmamusumeActivity" }

function Clean {
    Write-Host "-- Cleaning up"
    if (Test-Path $APK_EXTRACT_DIR) { Remove-Item -Recurse -Force $APK_EXTRACT_DIR }
#     if (Test-Path $TMP_BASE_APK) { Remove-Item $TMP_BASE_APK -Force }
#     if (Test-Path $TMP_CONFIG_APK) { Remove-Item $TMP_CONFIG_APK -Force }
}

if ($args[0] -eq "clean") {
    Clean
    exit
}

# Debug / Release
if ($env:RELEASE -eq "1") {
    $BUILD_TYPE = "release"
} else {
    $BUILD_TYPE = "debug"
}

# Arguments:
#   $args[0] = keystore
#   $args[1] = base.apk
#   $args[2] = config.apk
if (-not (Test-Path $args[0])) { Write-Error "Keystore doesn't exist"; exit 1 }
if (-not (Test-Path $args[1])) { Write-Error "Base APK doesn't exist"; exit 1 }
if (-not (Test-Path $args[2])) { Write-Error "Config APK doesn't exist"; exit 1 }

if (-not $env:APKSIGNER) {
    Write-Error "APKSIGNER must be set"
    exit 1
}

# --- Build Rust library ---
Write-Host "-- Building"
powershell -File ./tools/android/powershell/build.ps1

# --- Extract APK ---
Clean

Write-Host "-- Extracting config APK"
if (Test-Path $APK_EXTRACT_DIR) { Remove-Item -Recurse -Force $APK_EXTRACT_DIR }
New-Item -ItemType Directory -Force -Path $APK_EXTRACT_DIR | Out-Null

# unzip
# Expand-Archive -Path $args[2] -DestinationPath $APK_EXTRACT_DIR -Force
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::ExtractToDirectory($args[2], $APK_EXTRACT_DIR)

# --- Detect arch ---
if (Test-Path $APK_ARM64_LIB_DIR) {
    $BUILD_DIR = "./build/aarch64-linux-android"
    $APK_LIB_DIR = $APK_ARM64_LIB_DIR
} elseif (Test-Path $APK_ARM_LIB_DIR) {
    $BUILD_DIR = "./build/armv7-linux-androideabi"
    $APK_LIB_DIR = $APK_ARM_LIB_DIR
} elseif (Test-Path $APK_X86_LIB_DIR) {
    $BUILD_DIR = "./build/i686-linux-android"
    $APK_LIB_DIR = $APK_X86_LIB_DIR
} elseif (Test-Path $APK_X86_64_LIB_DIR) {
    $BUILD_DIR = "./build/x86_64-linux-android"
    $APK_LIB_DIR = $APK_X86_64_LIB_DIR
} else {
    Write-Error "-- Failed to detect config architecture!"
    exit
}

Write-Host "-- Detected lib dir: $APK_LIB_DIR"

# --- Backup original ---
if (-not (Test-Path (Join-Path $APK_LIB_DIR "libmain_orig.so"))) {
    Write-Host "-- Copying libmain_orig.so"
    Copy-Item (Join-Path $APK_LIB_DIR "libmain.so") (Join-Path $APK_LIB_DIR "libmain_orig.so") -Force
}

# --- Replace with Hachimi ---
Write-Host "-- Copying Hachimi"
Copy-Item "$BUILD_DIR\$BUILD_TYPE\libhachimi.so" "$APK_LIB_DIR\libmain.so" -Force

# --- Repack APK ---
Write-Host "-- Repacking config APK"

if (Test-Path $TMP_CONFIG_APK) { Remove-Item $TMP_CONFIG_APK -Force }
if (Test-Path $TMP_BASE_APK) { Remove-Item $TMP_BASE_APK -Force }



Push-Location $APK_EXTRACT_DIR
& "$env:SevenZip" a -tzip -mx=6 "$TMP_CONFIG_APK" *

Pop-Location



# --- Signing ---
Write-Host "-- Signing APKs"
Write-Host "(Password is securep@ssw0rd816-n if using UmaPatcher keystore)"

Copy-Item $args[1] $TMP_BASE_APK -Force

& $env:APKSIGNER sign --ks $args[0] $TMP_BASE_APK
& $env:APKSIGNER sign --ks $args[0] $TMP_CONFIG_APK

# --- Installing ---
Write-Host "-- Installing"

adb shell am force-stop $env:PACKAGE_NAME
adb install-multiple $TMP_BASE_APK $TMP_CONFIG_APK

Clean

# --- Launching ---
Write-Host "-- Launching"
adb shell am start-activity "$($env:PACKAGE_NAME)/$($env:ACTIVITY_NAME)"

# --- Logcat ---
Write-Host "-- Logcat"
adb logcat | Select-String -Pattern "Hachimi"
