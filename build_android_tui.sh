#!/bin/sh
export API_LEVEL=33  # Android 13, safe choice for modern phoneso

export TOOLCHAIN=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64
export TARGET=aarch64-linux-android
export AR=$TOOLCHAIN/bin/llvm-ar
export CC=$TOOLCHAIN/bin/${TARGET}${API_LEVEL}-clang
export CXX=$TOOLCHAIN/bin/${TARGET}${API_LEVEL}-clang++
export RANLIB=$TOOLCHAIN/bin/llvm-ranlib
export STRIP=$TOOLCHAIN/bin/llvm-strip

export AWS_LC_SYS_CROSS_COMPILE=1
export CMAKE_TOOLCHAIN_FILE=$NDK_HOME/build/cmake/android.toolchain.cmake
export ANDROID_ABI=arm64-v8a
export ANDROID_PLATFORM=android-$API_LEVEL

cargo build \
  --release \
  --target aarch64-linux-android \
  -p sickgnal_tui