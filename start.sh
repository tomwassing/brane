#!/bin/bash

# START.sh
#   by Lut99
#
# Created:
#   19 Jan 2022, 13:52:03
# Last edited:
#   01 Feb 2022, 14:05:02
# Auto updated?
#   Yes
#
# Description:
#   File that launches the given executable after compiling it on the /build
#   volume
#

# Read the CLI
if [ $# == 2 ]; then
    # Check if a command was given
    if [ $2 == "stall" ]; then
        # Stall for a long time
        sleep 1d
    else
        echo "Unknown command '$2'; options are:"
        echo " - 'stall'"
        exit -1
    fi
elif [ $# != 1 ]; then
    # Otherwise, we no liky
    echo "usage: ./start.sh <brane-exec>"
    exit -1
fi
brane_exec=$1

# Copy the output from the shared build to the root
cd /
cp "/build/target/containers/target/release/$brane_exec" /

# Run it
"./$brane_exec" --debug
