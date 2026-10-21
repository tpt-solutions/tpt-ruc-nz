#!/usr/bin/env sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 TPT Solutions
#
# UNTESTED. This script has not been run. It needs the Android NDK and the
# cargo-ndk tool (check its licence with the owner before installing it).
#
# Builds libtpt_ruc_ffi_android.so for the common Android ABIs and copies the
# results into the sample app's jniLibs folder.
set -eu

cd "$(dirname "$0")/../.."

rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

cargo ndk \
  -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o samples/android/app/src/main/jniLibs \
  build --release -p tpt-ruc-ffi-android
