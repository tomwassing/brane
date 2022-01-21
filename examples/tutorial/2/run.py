#!/usr/bin/env python3
import base64
import os
import sys
import yaml

def decode(s: str) -> str:
    s = s.replace("\n", "")
    b = base64.b64decode(s);
    return b.decode("utf-8")

def encode(s: str) -> str:
    b = s.encode("utf-8")
    b = base64.b64encode(b);
    return b.decode("utf-8")


FUNCTIONS = {
    "decode": decode,
    "encode": encode,
}

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in FUNCTIONS:
        print(f"Usage: {sys.argv[0]} encode|decode <value>")
        exit(1)
    
    command = sys.argv[1]
    value = os.environ["INPUT"]
    output = FUNCTIONS[command](value)
    print(yaml.dump({ "output": output }))
