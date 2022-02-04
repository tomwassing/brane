#!/bin/bash

# BUILD.sh
#   by Lut99
#
# Created:
#   20 Jan 2022, 10:35:38
# Last edited:
#   04 Feb 2022, 11:21:21
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

elif [[ $# -ge 1 && $1 == "build_openssl" ]]; then
    # Create the musl binary directories with links
    ln -s /usr/include/x86_64-linux-gnu/asm /usr/include/x86_64-linux-musl/asm
    ln -s /usr/include/asm-generic /usr/include/x86_64-linux-musl/asm-generic
    ln -s /usr/include/linux /usr/include/x86_64-linux-musl/linux
    mkdir /musl

    # Get the source
    wget https://github.com/openssl/openssl/archive/OpenSSL_1_1_1f.tar.gz
    tar zxvf OpenSSL_1_1_1f.tar.gz 
    cd openssl-OpenSSL_1_1_1f/
    
    # Configure the project
    CC="musl-gcc -fPIE -pie" ./Configure no-shared no-async --prefix=/musl --openssldir=/musl/ssl linux-x86_64

    # Compile it
    make depend
    make -j$(nproc)
    make install

    # Done, copy the resulting folder to the build one
    mkdir -p /build/contrib/deps/openssl
    cp -r /musl/include /build/contrib/deps/openssl/
    cp -r /musl/lib /build/contrib/deps/openssl/

elif [[ $# -ge 1 && $1 == "bash" ]]; then
    # Just open bash
    /bin/bash

elif [[ $# -ge 1 ]]; then
    # Illegal command given
    echo "Unknown command '$1'"
    exit -1

else
    # No command given
    echo "usage: $0 <command>"
    exit -1

fi
