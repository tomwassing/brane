#!/bin/bash

# CONTAINER BUILD SSL.sh
#   by Lut99
#
# Created:
#   20 Jan 2022, 10:35:38
# Last edited:
#   07 May 2022, 15:42:06
# Auto updated?
#   Yes
#
# Description:
#   Script that builds a quick OpenSSL library to build against for a musl
#   target.
#


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

# Compile it (but not the docs)
make depend
make -j$(nproc)
make install_sw install_ssldirs

# Done, copy the resulting folder to the build one
mkdir -p /build/target/openssl
cp -r /musl/include /build/target/openssl/
cp -r /musl/lib /build/target/openssl/
