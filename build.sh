#!/bin/bash

# BUILD.sh
#   by Lut99
#
# Created:
#   20 Jan 2022, 10:35:38
# Last edited:
#   20 Jan 2022, 10:52:31
# Auto updated?
#   Yes
#
# Description:
#   Script that builds the brane project in a container.
#

# Compile in the share
cd /build
CARGO_HOME="/build/target/containers/cache" cargo build \
    --release \
    --target-dir "/build/target/containers/target" \
    --package "brane-api" \
    --package "brane-clb" \
    --package "brane-drv" \
    --package "brane-job" \
    --package "brane-log" \
    --package "brane-plr"
