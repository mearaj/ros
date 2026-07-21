# Release signing material

Private production signing identities for ROS. **Never commit keys or passwords.**

Canonical packaging and key-generation steps:
[docs/runbooks/release-packaging.md](../../docs/runbooks/release-packaging.md).

```text
secrets/signing/
  android/ros-release.p12 + key.properties   # Android APK
  windows/ros-codesign.pfx + password        # Authenticode (Windows host)
  gpg/gotigin-ros-release.sec                # Website .sig (all platforms)
```

Public key (safe to commit): `ros-website/public/keys/gotigin-ros-release.pub.asc`.
