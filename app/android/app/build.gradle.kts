plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "dev.kogical.colecto"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "com.kogical.colecto"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signing with the debug keys for now, so `flutter run --release` works.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs(layout.buildDirectory.dir("generated/colectoJniLibs"))
        }
    }
}

val copyColectoFfi by tasks.registering {
    val targetRoot = rootProject.projectDir.resolve("../target")
    val outDir = layout.buildDirectory.dir("generated/colectoJniLibs")
    val abiMap = mapOf(
        "aarch64-linux-android" to "arm64-v8a",
        "x86_64-linux-android" to "x86_64",
    )
    doLast {
        val destRoot = outDir.get().asFile
        abiMap.forEach { (triple, abi) ->
            val releaseLib = targetRoot.resolve("$triple/release/libcolecto_ffi.so")
            val debugLib = targetRoot.resolve("$triple/debug/libcolecto_ffi.so")
            val src = when {
                releaseLib.exists() -> releaseLib
                debugLib.exists() -> debugLib
                else -> null
            }
            if (src != null) {
                val abiDir = destRoot.resolve(abi)
                abiDir.mkdirs()
                src.copyTo(abiDir.resolve("libcolecto_ffi.so"), overwrite = true)
            }
        }
    }
}

tasks.named("preBuild") {
    dependsOn(copyColectoFfi)
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
