#Requires -Version 5.0
$ErrorActionPreference = "Stop"

# Detect OS
$OS = "windows"

# Check NDK
if (-not $env:ANDROID_NDK_ROOT) {
    Write-Error "ANDROID_NDK_ROOT must be set"
    exit 1
}

# Debug / Release
if ($env:RELEASE -eq "1") {
    if ($env:CARGOARGS) {
        $CARGOARGS = "$env:CARGOARGS --release"
    } else {
        $CARGOARGS = "--release"
    }
    $BUILD_TYPE = "release"
} else {
    $BUILD_TYPE = "debug"
}

$TOOLCHAIN_DIR = "$env:ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/windows-x86_64"
$SYSROOT = "$TOOLCHAIN_DIR/sysroot"

$env:BINDGEN_EXTRA_CLANG_ARGS = "--sysroot=$SYSROOT"
$env:RUSTFLAGS = "-C link-args=-static-libstdc++ -C link-args=-lc++abi"

# Set toolchains
$env:CC_aarch64_linux_android = "$TOOLCHAIN_DIR/bin/aarch64-linux-android24-clang.cmd"
$env:CXX_aarch64_linux_android = "$TOOLCHAIN_DIR/bin/aarch64-linux-android24-clang++.cmd"
$env:AR_aarch64_linux_android = "$TOOLCHAIN_DIR/bin/llvm-ar.exe"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$TOOLCHAIN_DIR/bin/aarch64-linux-android24-clang.cmd"

# mkdir build
if (-not (Test-Path build)) { mkdir build | Out-Null }

# Cargo build
cargo build --target=aarch64-linux-android --target-dir=build $CARGOARGS

# Copy .so
Copy-Item "build\aarch64-linux-android\$BUILD_TYPE\libhachimi.so" "build\libmain-arm64-v8a.so" -Force

# SHA1 for release
if ($env:RELEASE -eq "1") {
    $ARM64_V8A_SHA1 = (Get-FileHash -Algorithm SHA1 "build\libmain-arm64-v8a.so").Hash.ToLower()

@"
{
    "libmain-arm64-v8a.so": "$ARM64_V8A_SHA1"
}
"@ | Out-File -Encoding utf8 "build\sha1.json"
}
