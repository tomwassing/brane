#!/usr/bin/env python3

# HELP.py
#   by Lut99
#
# Created:
#   18 Jan 2022, 15:25:34
# Last edited:
#   20 Jan 2022, 13:48:02
# Auto updated?
#   Yes
#
# Description:
#   A script containing brane functions for showing the disk and other useful
#   tools.
#

import subprocess
import os
import sys
import time
import yaml


##### LIBRARY FUNCTIONS #####
def cp(source, target, args):
    """
        Copies the source to the target, and outputs 'success' if so or stderr if not.
    """

    # Prepare the call and do it
    p = subprocess.Popen(["cp", source, target] + args, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    _, err = p.communicate()

    # Construct the output message
    output = "success"
    if len(err) > 0:
        output = "Stderr:\n" + err.decode("utf-8")
    
    # Return it
    print(yaml.dump({"output": output}))

    # Done
    return 0

def ls(path, args):
    """
        Returns the output of the given ls command in YAML format to stdout.
    """

    # Prepare the call and do it
    p = subprocess.Popen(["ls", path] + args, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    out, err = p.communicate()

    # Construct the output message
    output = out.decode("utf-8") if len(out) > 0 else '<no output>'
    if len(err) > 0:
        output += "\n\nStderr:\n" + err.decode("utf-8")

    # Return it
    print(yaml.dump({"output": output}))

    # Done
    return 0

def cat(path, args):
    """
        Returns the contents of the given file.
    """

    # Prepare the call and do it
    p = subprocess.Popen(["cat", path] + args, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    out, err = p.communicate()

    # Construct the output message
    output = out.decode("utf-8") if len(out) > 0 else '<no output>'
    if len(err) > 0:
        output += "\n\nStderr:\n" + err.decode("utf-8")

    # Return it
    print(yaml.dump({"output": output}))

    # Done
    return 0

def stall(n_seconds):
    """
        Stalls for the given number of seconds using a busy loop.
    """

    now = time.time()
    while time.time() - now < n_seconds: pass

    # Return the success thingy
    print(yaml.dump({"output": "success"}))

    # Done
    return 0





##### ENTRY POINT #####
if __name__ == "__main__":
    # Read the command
    if len(sys.argv) <= 1:
        print("No command specified; nothing to do.", file=sys.stderr)
        exit(-1)
    command = sys.argv[1]

    # Switch on the command
    if command == "cp":
        # Parse the path to cp from and to
        source = os.environ["SOURCE"]
        target = os.environ["TARGET"]

        # Pass it to the function
        exit(cp(source, target, []))

    elif command == "ls":
        # Parse the path to ls and which options to ls
        path = os.environ["DIRECTORY"]

        # Pass to the function
        exit(ls(path, []))

    elif command == "cat":
        # Parse the path
        path = os.environ["FILE"]

        # Pass to the function
        exit(cat(path, []))

    elif command == "stall":
        # Parse the number of seconds
        n_seconds = os.environ["NSECONDS"]
        try:
            n_seconds = int(n_seconds)
        except ValueError as e:
            print(f"Could not parse NSECONDS '{os.environ['NSECONDS']}' as int: {e}.", file=sys.stderr)

        # Run the thing
        exit(stall(n_seconds))
