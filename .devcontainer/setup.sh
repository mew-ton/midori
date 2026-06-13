#!/usr/bin/env bash
# Container build-dependency setup. Invoked once at container creation via the
# devcontainer onCreateCommand. Idempotent: safe to re-run.
#
# midori-driver-midi -> midir -> alsa-sys links against ALSA at build time and
# needs the dev headers (alsa.pc, located via pkg-config). Without them
# `cargo build`/`cargo test` fails for the whole workspace, not just the MIDI
# crate. The base rust devcontainer image does not ship these headers.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

sudo apt-get update
sudo apt-get install -y --no-install-recommends libasound2-dev
