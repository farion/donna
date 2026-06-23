# Donna

Donna is a single-user, local-first personal work-life assistant built with Rust
and egui.

## Local Verification

Install Rust through `rustup`; this repository uses the checked-in
`rust-toolchain.toml` with stable Rust, `rustfmt`, and `clippy`.

On Debian or Ubuntu desktops and GitHub Actions runners, install the native
desktop build packages:

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libgl1-mesa-dev \
  libwayland-dev \
  libx11-dev \
  libxcb-keysyms1-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxcb1-dev \
  libxi-dev \
  libxkbcommon-dev \
  libxrandr-dev \
  pkg-config
```

Run the same verification sequence used by CI:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

`cargo build --locked` verifies the Linux desktop binary path. The CI workflow
does not require secrets and is safe to roll back by reverting the workflow and
toolchain/docs changes.
