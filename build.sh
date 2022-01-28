#!/bin/bash

# BUILD.sh
#   by Lut99
#
# Created:
#   20 Jan 2022, 10:35:38
# Last edited:
#   28 Jan 2022, 16:41:36
# Auto updated?
#   Yes
#
# Description:
#   Script that builds the brane project in a container.
#

# If we're given 'get_target', output the docker cargo target instead of building
if [[ $# -ge 1 && $1 == "get_target" ]]; then
    target=$(rustc -vV | sed -n 's|host: ||p')
    echo "$target"
    exit 0

elif [[ $# -ge 1 && $1 == "build_brane" ]]; then
    # Compile the framework in the share
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

elif [[ $# -ge 1 && $1 == "build_branelet" ]]; then
    # Compile the branelet binary in the share
    cd /build
    CARGO_HOME="/buil/target/containers/cache" cargo build \
        --release \
        --target-dir "/build/target/containers/target" \
        --package "brane-let"

elif [[ $# -ge 1 ]]; then
    # Illegal command given
    echo "Unknown command '$1'"
    exit -1

else
    # No command given
    echo "usage: $0 <command>"
    exit -1

fi
