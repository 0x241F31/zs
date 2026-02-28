set -euo pipefail

rm -rf .zs

export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=".dev/android-sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android35-clang"
export CC_x86_64_linux_android=".dev/android-sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/linux-x86_64/bin/clang"
export CXX_x86_64_linux_android=".dev/android-sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/linux-x86_64/bin/clang++"
export AR_x86_64_linux_android=".dev/android-sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
export RANLIB_x86_64_linux_android=".dev/android-sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"

export CFLAGS_x86_64_linux_android="--target=x86_64-linux-android35 \
--sysroot=.dev/android-sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
export CXXFLAGS_x86_64_linux_android="$CFLAGS_x86_64_linux_android"

cargo build --target=x86_64-linux-android

JDK_DIR=".dev/java-25-openjdk"

cargo run --manifest-path=../Cargo.toml -- \
	fetch \
	--coursier /usr/bin/coursier \
	--output .zs/deps \
	androidx.webkit:webkit:1.5.0

cargo run --manifest-path=../Cargo.toml -- \
	setup sysmodules \
	--jdk $JDK_DIR \
	--android-platform .dev/android-sdk/platforms/android-36.1 \
	--workdir .zs/tmp \
	--output .zs/sysmodules

cargo run --manifest-path=../Cargo.toml -- \
	build \
	--jdk $JDK_DIR \
	--android-platform .dev/android-sdk/platforms/android-36.1 \
	--android-build-tools .dev/android-sdk/build-tools/36.1.0 \
	--arch amd64 \
	--manifest AndroidManifest.xml \
	--package com.example.zs \
	--resources resource \
	--sysmodules .zs/sysmodules \
	--lib target/x86_64-linux-android/debug/libmain.so \
	--java java/MainActivity.java \
	--jar .zs/deps/androidx.webkit/webkit/1.5.0/source/classes.jar \
	--jar .zs/deps/androidx.annotation/annotation/1.2.0/annotation-1.2.0.jar \
	--jar .zs/deps/androidx.arch.core/core-common/2.0.0/core-common-2.0.0.jar \
	--jar .zs/deps/androidx.collection/collection/1.0.0/collection-1.0.0.jar \
	--jar .zs/deps/androidx.core/core/1.1.0/source/classes.jar \
	--jar .zs/deps/androidx.lifecycle/lifecycle-common/2.0.0/lifecycle-common-2.0.0.jar \
	--jar .zs/deps/androidx.lifecycle/lifecycle-runtime/2.0.0/source/classes.jar \
	--jar .zs/deps/androidx.versionedparcelable/versionedparcelable/1.1.0/source/classes.jar

cargo run --manifest-path=../Cargo.toml -- \
	setup keystore \
	--keytool $JDK_DIR/bin/keytool \
	--output .zs/debug.keystore

cargo run --manifest-path=../Cargo.toml -- \
	sign .zs/build/app.apk \
		--android-build-tools .dev/android-sdk/build-tools/36.1.0 \
		--keystore .zs/debug.keystore

cargo run --manifest-path=../Cargo.toml -- \
	deploy .zs/build/app.apk --package com.example.zs --launch
