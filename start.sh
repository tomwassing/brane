#!/bin/bash

# START.sh
#   by Lut99
#
# Created:
#   19 Jan 2022, 13:52:03
# Last edited:
#   21 Jan 2022, 14:27:56
# Auto updated?
#   Yes
#
# Description:
#   File that launches the given executable after compiling it on the /build
#   volume
#

# Read the CLI
if [ $# != 1 ]; then
    echo "usage: ./start.sh <brane-exec>"
    exit -1
fi
brane_exec=$1

# Copy the output from the shared build to the root
cd /
cp "/build/target/containers/target/release/$brane_exec" /

# Run it
"./$brane_exec" --debug
