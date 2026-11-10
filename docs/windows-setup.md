# Windows contributor setup

This guide targets a 64-bit Windows machine using the Rust MSVC toolchain. The
checker is read-only: it does not install software, build the project, start a
window or use the GPU.

1. Install [Git for Windows](https://git-scm.com/install/windows),
   [Python 3](https://www.python.org/downloads/windows/) and
   [Rust via rustup](https://rustup.rs/). Select the
   `x86_64-pc-windows-msvc` Rust toolchain. Add `rustfmt` and `clippy` with
   `rustup component add rustfmt clippy` if they are missing.
2. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   or Visual Studio with **Desktop development with C++**, including the MSVC
   x64/x86 build tools and a Windows 10 or 11 SDK.
3. Reopen PowerShell in the repository and run:

   ```powershell
   .\scripts\check-setup.ps1
   .\scripts\check-cpu.ps1
   ```

The first command reports missing prerequisites and exits with code 1 when a
required check fails; it exits 0 when they are present. It checks executable
availability, Rust's host, an installed x64 linker and matching SDK header and
library versions. This is a diagnostic, not a compile or link test; the CPU
check is the project validation. The checker currently targets x64 MSVC and
uses Visual Studio Installer's `vswhere` and the Windows Kits registry key;
custom or ARM toolchains may need manual verification.

GitHub CLI is optional for local CPU development. To publish a PR through the
project's [factory workflow](software-factory.md), install
[GitHub CLI](https://cli.github.com/) and authenticate with `gh auth login`.
Use `.\scripts\check-setup.ps1 -RequireGhAuth` to make that check required. The
checker suppresses authentication output and does not print credentials. It
checks the CLI's authentication status, not repository permissions.

For desktop type changes, `cargo build --locked -j 2` compiles without launching
the interface. GPU tests and window checks are opt-in and require coordination
with the machine's owner. Keep local checks to two CPU jobs/test threads.
