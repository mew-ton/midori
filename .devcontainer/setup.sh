#!/usr/bin/env bash
# Container build-dependency setup. Invoked once at container creation via the
# devcontainer onCreateCommand. Idempotent: safe to re-run.
#
# Two groups of system packages are installed:
#
# 1. ALSA dev headers (libasound2-dev): midori-driver-midi -> midir ->
#    alsa-sys links against ALSA at build time and needs the dev headers
#    (alsa.pc, located via pkg-config). Without them `cargo build`/`cargo test`
#    fails for the whole workspace, not just the MIDI crate.
#
# 2. Electron runtime libraries + a virtual X server (Xvfb/xauth): packages/gui
#    is an Electron app whose prebuilt Chromium binary dynamically links GTK,
#    NSS, X11, GBM/DRM, ATK, CUPS, D-Bus, and xkbcommon. The container is
#    headless, so Xvfb provides an off-screen display, launched via
#    `xvfb-run electron .`. Without these the GUI cannot be started or verified
#    inside the container.
#
# The base rust devcontainer image ships none of these.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

# Retry on transient network failures: onCreateCommand runs once during container
# build, where a single dropped connection would otherwise fail the whole setup.
sudo apt-get -o Acquire::Retries=3 update
sudo apt-get -o Acquire::Retries=3 install -y --no-install-recommends \
  libasound2-dev \
  xvfb \
  xauth \
  libgtk-3-0t64 \
  libnss3 \
  libnspr4 \
  libgbm1 \
  libdrm2 \
  libxcomposite1 \
  libxdamage1 \
  libxfixes3 \
  libxrandr2 \
  libatk1.0-0t64 \
  libatk-bridge2.0-0t64 \
  libatspi2.0-0t64 \
  libcups2t64 \
  libdbus-1-3 \
  libxkbcommon0

# Activate the pnpm version pinned in package.json's `packageManager` field.
# Corepack installs a `pnpm` shim that takes precedence over any pnpm bundled
# with the Node feature, so every contributor and CI resolve the same package
# manager (pnpm 11, which requires Node >= 22.13 — satisfied by the Node 24
# feature above).
corepack enable
