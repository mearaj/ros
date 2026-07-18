import java.util.Properties

val releaseSigningProperties = Properties()
val releaseSigningPropertiesFile = rootProject.file("key.properties")
val releaseSigningRequiredKeys = listOf("storeFile", "storePassword", "keyAlias", "keyPassword")

if (releaseSigningPropertiesFile.isFile) {
    releaseSigningPropertiesFile.inputStream().use(releaseSigningProperties::load)
}

val missingReleaseSigningKeys = releaseSigningRequiredKeys.filter {
    releaseSigningProperties.getProperty(it).isNullOrBlank()
}
val hasReleaseSigningConfig =
    releaseSigningPropertiesFile.isFile && missingReleaseSigningKeys.isEmpty()
val releaseSigningGateMessage = when {
    !releaseSigningPropertiesFile.isFile ->
        "Release packaging is blocked: create android/key.properties from " +
            "android/key.properties.example and use a controlled signing identity."
    else ->
        "Release packaging is blocked: android/key.properties is missing " +
            missingReleaseSigningKeys.joinToString(", ") + "."
}

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "com.gotigin.ros"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // Stable reverse-DNS application identity for Restaurant Operating System.
        applicationId = "com.gotigin.ros"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        if (hasReleaseSigningConfig) {
            create("release") {
                storeFile = rootProject.file(releaseSigningProperties.getProperty("storeFile"))
                storePassword = releaseSigningProperties.getProperty("storePassword")
                keyAlias = releaseSigningProperties.getProperty("keyAlias")
                keyPassword = releaseSigningProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Never fall back to the Android debug key for a release artifact.
            if (hasReleaseSigningConfig) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
}

// `signingConfig = null` would create an unsigned artifact. Block every
// release task when the accountable signing configuration is absent or
// incomplete, while leaving Development tasks fully usable.
if (!hasReleaseSigningConfig) {
    tasks.configureEach {
        if (name.contains("Release")) {
            doFirst {
                throw GradleException(releaseSigningGateMessage)
            }
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
