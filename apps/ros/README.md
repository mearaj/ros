# Restaurant Operating System Flutter Client

This is the adaptive Flutter/Dart client for Gotigin Restaurant Operating
System. It targets desktop POS terminals, tablets, and mobile devices.

The application communicates with the trusted Rust core through generated
flutter_rust_bridge bindings located under lib/src/rust. Do not place
financial, authorization, migration, or database rules solely in Dart.

## Development

    flutter analyze
    flutter test
    flutter build linux --debug

After changing Rust APIs in rust/src/api, regenerate the Dart binding:

    flutter_rust_bridge_codegen generate

Run that command from this directory and commit generated binding changes with
the Rust API change.

For a clean, unpublished local installation, use the confirmation-gated
development reset script from the repository root. It clears the matching
encrypted development database and secure-store credential together; it is not
for a production/customer install. See
[`docs/runbooks/local-development.md`](../../docs/runbooks/local-development.md#reset-an-unpublished-local-installation).
