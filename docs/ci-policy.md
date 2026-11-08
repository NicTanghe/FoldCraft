# Continuous integration

The CPU, cross-platform build and Windows portable preview workflows run on
pull requests, pushes to `main` and manual dispatch. Pull-request updates cancel
superseded runs for that PR. Builds and tests run without task-catalog checks
or metadata-only bypasses.

CPU checks cover the Python queue tests and queue validation, Rust formatting,
tests, Clippy and headless template validation.
Desktop checks compile and test desktop code without opting into GPU tests.
Cross-platform builds package macOS, Linux and Windows binaries. The Windows
portable workflow also checks packaging and native editor launch/capture.
Dependency caches do not replace validation.

These workflows provide feedback for the [feature PR workflow](software-factory.md).
Every feature is merged through a PR using `gh`; no upstream coordinator is used.
Any GitHub branch protections or rulesets
are separate repository settings and are not changed by these files.
