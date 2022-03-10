#!/bin/bash
# SLEEP.sh
#   by Lut99
#
# Created:
#   10 Mar 2022, 17:32:39
# Last edited:
#   10 Mar 2022, 17:34:24
# Auto updated?
#   Yes
#
# Description:
#   Simple script that sleeps for the given amout of seconds.
#

# Simply call sleep with the proper environment variable
sleep "$TIMEOUT"

# Echo the result
echo "output: Awake after $TIMEOUT seconds!"
