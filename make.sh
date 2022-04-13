#!/bin/bash
# MAKE.sh
#   by Lut99
#
# Created:
#   03 Mar 2022, 17:03:04
# Last edited:
#   04 Apr 2022, 10:34:06
# Auto updated?
#   Yes
#
# Description:
#   Custom "Makefile" for the Brane project.
#   Not using GNU Make because it doesn't really understand the concept of
#   not rebuilding images when not needed.
#


# Lists the generated targets of OpenSSL
OPENSSL_DIR="$(pwd)/target/openssl"
OPENSSL_TARGETS=("$OPENSSL_DIR/lib/libcrypto.a" "$OPENSSL_DIR/lib/libssl.a" \
                "$OPENSSL_DIR/lib/pkgconfig/libcrypto.pc" "$OPENSSL_DIR/lib/pkgconfig/libssl.pc" "$OPENSSL_DIR/lib/pkgconfig/openssl.pc"
                "$OPENSSL_DIR/include/openssl/aes.h" "$OPENSSL_DIR/include/openssl/asn1err.h" "$OPENSSL_DIR/include/openssl/asn1.h"
                "$OPENSSL_DIR/include/openssl/asn1_mac.h" "$OPENSSL_DIR/include/openssl/asn1t.h" "$OPENSSL_DIR/include/openssl/asyncerr.h"
                "$OPENSSL_DIR/include/openssl/async.h" "$OPENSSL_DIR/include/openssl/bioerr.h" "$OPENSSL_DIR/include/openssl/bio.h"
                "$OPENSSL_DIR/include/openssl/blowfish.h" "$OPENSSL_DIR/include/openssl/bnerr.h" "$OPENSSL_DIR/include/openssl/bn.h"
                "$OPENSSL_DIR/include/openssl/buffererr.h" "$OPENSSL_DIR/include/openssl/buffer.h" "$OPENSSL_DIR/include/openssl/camellia.h"
                "$OPENSSL_DIR/include/openssl/cast.h" "$OPENSSL_DIR/include/openssl/cmac.h" "$OPENSSL_DIR/include/openssl/cmserr.h"
                "$OPENSSL_DIR/include/openssl/cms.h" "$OPENSSL_DIR/include/openssl/comperr.h" "$OPENSSL_DIR/include/openssl/comp.h"
                "$OPENSSL_DIR/include/openssl/conf_api.h" "$OPENSSL_DIR/include/openssl/conferr.h" "$OPENSSL_DIR/include/openssl/conf.h"
                "$OPENSSL_DIR/include/openssl/cryptoerr.h" "$OPENSSL_DIR/include/openssl/crypto.h" "$OPENSSL_DIR/include/openssl/cterr.h"
                "$OPENSSL_DIR/include/openssl/ct.h" "$OPENSSL_DIR/include/openssl/des.h" "$OPENSSL_DIR/include/openssl/dherr.h"
                "$OPENSSL_DIR/include/openssl/dh.h" "$OPENSSL_DIR/include/openssl/dsaerr.h" "$OPENSSL_DIR/include/openssl/dsa.h"
                "$OPENSSL_DIR/include/openssl/dtls1.h" "$OPENSSL_DIR/include/openssl/ebcdic.h" "$OPENSSL_DIR/include/openssl/ecdh.h"
                "$OPENSSL_DIR/include/openssl/ecdsa.h" "$OPENSSL_DIR/include/openssl/ecerr.h" "$OPENSSL_DIR/include/openssl/ec.h"
                "$OPENSSL_DIR/include/openssl/engineerr.h" "$OPENSSL_DIR/include/openssl/engine.h" "$OPENSSL_DIR/include/openssl/e_os2.h"
                "$OPENSSL_DIR/include/openssl/err.h" "$OPENSSL_DIR/include/openssl/evperr.h" "$OPENSSL_DIR/include/openssl/evp.h"
                "$OPENSSL_DIR/include/openssl/hmac.h" "$OPENSSL_DIR/include/openssl/idea.h" "$OPENSSL_DIR/include/openssl/kdferr.h"
                "$OPENSSL_DIR/include/openssl/kdf.h" "$OPENSSL_DIR/include/openssl/lhash.h" "$OPENSSL_DIR/include/openssl/md2.h"
                "$OPENSSL_DIR/include/openssl/md4.h" "$OPENSSL_DIR/include/openssl/md5.h" "$OPENSSL_DIR/include/openssl/mdc2.h"
                "$OPENSSL_DIR/include/openssl/modes.h" "$OPENSSL_DIR/include/openssl/objectserr.h" "$OPENSSL_DIR/include/openssl/objects.h"
                "$OPENSSL_DIR/include/openssl/obj_mac.h" "$OPENSSL_DIR/include/openssl/ocsperr.h" "$OPENSSL_DIR/include/openssl/ocsp.h"
                "$OPENSSL_DIR/include/openssl/opensslconf.h" "$OPENSSL_DIR/include/openssl/opensslv.h" "$OPENSSL_DIR/include/openssl/ossl_typ.h"
                "$OPENSSL_DIR/include/openssl/pem2.h" "$OPENSSL_DIR/include/openssl/pemerr.h" "$OPENSSL_DIR/include/openssl/pem.h"
                "$OPENSSL_DIR/include/openssl/pkcs12err.h" "$OPENSSL_DIR/include/openssl/pkcs12.h" "$OPENSSL_DIR/include/openssl/pkcs7err.h"
                "$OPENSSL_DIR/include/openssl/pkcs7.h" "$OPENSSL_DIR/include/openssl/rand_drbg.h" "$OPENSSL_DIR/include/openssl/randerr.h"
                "$OPENSSL_DIR/include/openssl/rand.h" "$OPENSSL_DIR/include/openssl/rc2.h" "$OPENSSL_DIR/include/openssl/rc4.h"
                "$OPENSSL_DIR/include/openssl/rc5.h" "$OPENSSL_DIR/include/openssl/ripemd.h" "$OPENSSL_DIR/include/openssl/rsaerr.h"
                "$OPENSSL_DIR/include/openssl/rsa.h" "$OPENSSL_DIR/include/openssl/safestack.h" "$OPENSSL_DIR/include/openssl/seed.h"
                "$OPENSSL_DIR/include/openssl/sha.h" "$OPENSSL_DIR/include/openssl/srp.h" "$OPENSSL_DIR/include/openssl/srtp.h"
                "$OPENSSL_DIR/include/openssl/ssl2.h" "$OPENSSL_DIR/include/openssl/ssl3.h" "$OPENSSL_DIR/include/openssl/sslerr.h"
                "$OPENSSL_DIR/include/openssl/ssl.h" "$OPENSSL_DIR/include/openssl/stack.h" "$OPENSSL_DIR/include/openssl/storeerr.h"
                "$OPENSSL_DIR/include/openssl/store.h" "$OPENSSL_DIR/include/openssl/symhacks.h" "$OPENSSL_DIR/include/openssl/tls1.h"
                "$OPENSSL_DIR/include/openssl/tserr.h" "$OPENSSL_DIR/include/openssl/ts.h" "$OPENSSL_DIR/include/openssl/txt_db.h"
                "$OPENSSL_DIR/include/openssl/uierr.h" "$OPENSSL_DIR/include/openssl/ui.h" "$OPENSSL_DIR/include/openssl/whrlpool.h"
                "$OPENSSL_DIR/include/openssl/x509err.h" "$OPENSSL_DIR/include/openssl/x509.h" "$OPENSSL_DIR/include/openssl/x509v3err.h"
                "$OPENSSL_DIR/include/openssl/x509v3.h" "$OPENSSL_DIR/include/openssl/x509_vfy.h")





# Helper function that executes a recursive script call
make_target() {
    # Make sure there is only one target
    if [[ "$#" -ne 1 ]]; then
        echo "Usage: make_target <target>"
        exit 1
    fi

    # Run the recursive call with the error check
    ./make.sh "$1" || exit $?
}

# Helper function that executes a build step
exec_step() {
    # Construct a string from the input to show to user
    local cmd=""
    for arg in "$@"; do
        if [[ "$arg" =~ \  ]]; then
            cmd="$cmd \"$arg\""
        else
            cmd="$cmd $arg"
        fi
    done
    echo " >$cmd"

    # Run the recursive call with the error check
    "$@" || exit $?
}





# Read the target from the CLI
if [[ $# -eq 1 ]]; then
    target=$1
elif [[ $# -eq 0 ]]; then
    target="all"
else
    echo "Usage: $0 [<target>]"
    exit 1
fi



### META TARGETS ###
# Build every relevant thing
if [[ "$target" == "all" ]]; then
    # Use recursive calls to deal with it
    make_target instance
    make_target branelet
    make_target cli

# Clean the standard build folder
elif [[ "$target" == "clean" ]]; then
    # Remove the target folder
    exec_step rm -rf ./target

# Clean the OpenSSL build
elif [[ "$target" == "clean_openssl" ]]; then
    # Remove the openssl folder
    exec_step rm -rf ./contrib/deps/openssl



### BINARIES ###
# Build the command-line interface 
elif [[ "$target" == "cli" ]]; then
    # Use cargo to build the project; it manages dependencies and junk
    exec_step cargo build --release --package brane-cli

    # Done
    echo "Compiled executeable \"brane\" to './target/release/brane'"

# Build the branelet executable by cross-compiling
elif [[ "$target" == "branelet" ]]; then
    # We let cargo sort out dependencies
    exec_step rustup target add x86_64-unknown-linux-musl
	exec_step cargo build --release --package brane-let --target x86_64-unknown-linux-musl

    # Copy the resulting executable to the output branelet
    exec_step mkdir -p ./target/containers/target/release/
    exec_step cp ./target/x86_64-unknown-linux-musl/release/branelet ./target/containers/target/release/

    # Done
	echo "Compiled package initialization binary \"branelet\" to './target/containers/target/release/branelet'"

# Build the branelet executable by containerization
elif [[ "$target" == "branelet-safe" ]]; then
    # Dependencies: build the build image first
    make_target build-image-dev

    # Otherwise, continue the build as normal (by running it in a container)
    exec_step docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(pwd):/build" brane-build-dev "build_branelet"

    # Restore permissions
	echo "Removing root ownership from target folder (might require sudo password)"
	exec_step sudo chown -R "$USER":"$USER" ./target

    # Done
	echo "Compiled package initialization binary \"branelet\" to './target/containers/target/release/branelet'"



### IMAGES ###
# Build the build image
elif [[ "$target" == "build-image-dev" ]]; then
    # Then, call upon Docker to build it (it tackles caches)
    exec_step docker build --load -t brane-build-dev -f Dockerfile_dev.build .

    # Done
    echo "Built build image to Docker Image 'brane-build-dev'"

# Build the regular images
elif [[ "${target: -6}" == "-image" ]]; then
    # Get the name of the image
    image_name="${target%-image}"

    # Call upon Docker to build it (building in release as normal does not use any caching other than the caching of the image itself, sadly)
    exec_step docker build --load -t "brane-$image_name" -f Dockerfile.$image_name .

    # Done
    echo "Built $image_name image to Docker Image 'brane-$image_name'"

# Build the dev version of the images
elif [[ "${target: -10}" == "-image-dev" ]]; then
    # Get the name of the image
    image_name="${target%-image-dev}"

    # Call upon Docker to build it (we let it deal with caching)
    exec_step docker build --load -t "brane-$image_name-dev" -f Dockerfile_dev.$image_name .

    # Done
    echo "Built $image_name development image to Docker Image 'brane-$image_name-dev'"

# Target that bundles all the normal images together
elif [[ "$target" == "images" ]]; then
    # Simply build the images
    make_target api-image
    make_target clb-image
    make_target drv-image
    make_target job-image
    make_target log-image
    make_target plr-image

# Target that bundles all the development images together
elif [[ "$target" == "images-dev" ]]; then
    # Simply build the images
    make_target api-image-dev
    make_target clb-image-dev
    make_target drv-image-dev
    make_target job-image-dev
    make_target log-image-dev
    make_target plr-image-dev



### INSTANCE HELPERS ###
# Makes sure the docker network for Brane is up and running
elif [[ "$target" == "ensure-docker-network" ]]; then
    # Only add it if it doesn't exist already
    if [ ! -n "$(docker network ls -f name=brane | grep brane)" ]; then
		exec_step docker network create brane
        echo "Created Docker network 'brane'"
    else
        echo "Docker network 'brane' already exists"
	fi;

# Makes sure that the required infrastructure files are there
elif [[ "$target" == "ensure-configuration" ]]; then
    # Check infra.yml
    if [[ -f ./infra.yml ]]; then
        echo "infra.yml exists"
    else
        echo "Missing 'infra.yml'; provide one before running the Brane instance" >&2
        exit 1
    fi

    # Check secrets.yml
    if [[ -f ./infra.yml ]]; then
        echo "secrets.yml exists"
    else
        echo "Missing 'secrets.yml'; provide one before running the Brane instance" >&2
        exit 1
    fi

# Starts the auxillary services
elif [[ "$target" == "start-svc" ]]; then
    # Use Docker compose to start them
    exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml up -d"
	exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml rm -f"

    # Done
    echo "Started auxillary Brane services"

# Stops the auxillary services
elif [[ "$target" == "stop-svc" ]]; then
    # Use Docker compose again
    exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml down"

    # Done
    echo "Stopped auxillary Brane services"



### INSTANCE ###
# Builds the instance (which is just building the normal images)
elif [[ "$target" == "instance" ]]; then
    make_target images
    echo "Built Brane instance as Docker images"

# Starts the Brane services (the normal images)
elif [[ "$target" == "start-brn" ]]; then
    # Use Docker compose to start them
    exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml up -d"

    # Done
    echo "Started Brane services"

# Stops the Brane services (the normal images)
elif [[ "$target" == "stop-brn" ]]; then
    # Use Docker compose again
    exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml down"

    # Done
    echo "Stopped Brane services"

# Starts the instance (from the normal images)
elif [[ "$target" == "start-instance" ]]; then
    # Build the instance first
    make_target instance

    # Ensure that everything is in order and start the auxillary services
    make_target ensure-docker-network
    make_target ensure-configuration
    make_target start-svc

    # Start Brane
    make_target start-brn

# Stops the instance (from the normal images)
elif [[ "$target" == "stop-instance" ]]; then
    # Stop Brane
    make_target stop-brn

    # Stop the auxillary services
    make_target stop-svc



### DEVELOPMENT INSTANCE ###
# Build OpenSSL
elif [[ "$target" == "openssl" ]]; then
    # Prepare the build image for the SSL
    make_target ssl-image-dev

    # Compile the OpenSSL library
    exec_step docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(pwd):/build" brane-ssl-dev "build_openssl"

    # Restore the permissions
	echo "Removing root ownership from target folder (might require sudo password)"
	exec_step sudo chown -R "$USER":"$USER" ./target

    # Done
	echo "Compiled OpenSSL library to 'target/openssl/'"

# Build the instance (by cross-compiling)
elif [[ "$target" == "instance-dev" ]]; then
    # Make sure the musl compilers are found
    if ! command -v musl-gcc &> /dev/null; then
        echo "musl-gcc not found; make sure the musl toolchain is installed and available in your PATH"
        exit 1
    elif ! command -v musl-g++ &> /dev/null; then
        echo "musl-g++ not found; make sure the musl toolchain is installed and available in your PATH"
        echo "(It might not provide musl-g++, though. In that case, simply link g++:"
        echo "   $ sudo ln -s /bin/g++ /bin/musl-g++"
        echo ")"
        exit 1
    fi

    # Build openssl only if any of the files is missing
    for target in "${OPENSSL_TARGETS[@]}"; do
        if [[ ! -f "$target" ]]; then
            make_target openssl
            break
        fi
    done

    # Build the instance images
    make_target images-dev

    # Prepare the cross-compilation target
    exec_step rustup target add x86_64-unknown-linux-musl

    # Compile the framework, pointing to the compiled OpenSSL library
    echo " > OPENSSL_DIR=\"$OPENSSL_DIR\" \\"
    echo "   OPENSSL_LIB_DIR=\"$OPENSSL_DIR\" \\"
    echo "   cargo build \\"
    echo "      --release \\"
	echo "      --target-dir \"./target/containers/target\" \\"
	echo "      --target x86_64-unknown-linux-musl \\"
	echo "      --package brane-api \\"
	echo "      --package brane-clb \\"
	echo "      --package brane-drv \\"
	echo "      --package brane-job \\"
	echo "      --package brane-log \\"
	echo "      --package brane-plr"
    OPENSSL_DIR="$OPENSSL_DIR" \
    OPENSSL_LIB_DIR="$OPENSSL_DIR/lib" \
    cargo build \
        --release \
        --target-dir "./target/containers/target" \
        --target x86_64-unknown-linux-musl \
        --package brane-api \
        --package brane-clb \
        --package brane-drv \
        --package brane-job \
        --package brane-log \
        --package brane-plr \
        || exit $?

    # Copy the results to the correct location
    exec_step mkdir -p ./target/containers/target/release/
	exec_step /bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-{api,clb,drv,job,log,plr} ./target/containers/target/release/

    # Done!
    echo "Compiled Brane instance to 'target/containers/target/release/'"

# Starts the Brane services (cross-compiled)
elif [[ "$target" == "start-brn-dev" ]]; then
    # Use Docker compose to start them
    exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml up -d"

    # Done
    echo "Started Brane services"

# Stops the Brane services (cross-compiled)
elif [[ "$target" == "stop-brn-dev" ]]; then
    # Use Docker compose again
    exec_step bash -c "COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml down"

    # Done
    echo "Stopped Brane services"

# Starts the instance (cross-compiled)
elif [[ "$target" == "start-instance-dev" ]]; then
    # Build the instance first
    make_target instance-dev

    # Ensure that everything is in order and start the auxillary services
    make_target ensure-docker-network
    make_target ensure-configuration
    make_target start-svc

    # Start Brane
    make_target start-brn-dev

# Builds, stops, then re-starts the instance (cross-compiled)
elif [[ "$target" == "rebuild-instance-dev" ]]; then
    # Make the instance first
    make_target instance-dev

    # Restart the relevant brane containers
    exec_step docker restart brane-api brane-clb brane-drv brane-job brane-log brane-plr

    # Done
    echo "Rebuild Brane instance"

# Stops the instance (cross-compiled)
elif [[ "$target" == "stop-instance-dev" ]]; then
    # Stop Brane
    make_target stop-brn-dev

    # Stop the auxillary services
    make_target stop-svc



### DEVELOPMENT INSTANCE CONTAINERIZED ###
# Build the instance (by containerization)
elif [[ "$target" == "instance-safe" ]]; then
    # Build the build image
    make_target build-image-dev

    # Use Docker to build it
    exec_step docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(pwd):/build" brane-build-dev "build_brane"

    # Remove
	echo "Removing root ownership from target folder (might require sudo password)"
	exec_step sudo chown -R "$USER":"$USER" ./target

    # Done
	echo "Compiled Brane instance to 'target/containers/target/release/'"

# Starts the instance (built in a container)
elif [[ "$target" == "start-instance-safe" ]]; then
    # Build the instance first
    make_target instance-safe

    # Ensure that everything is in order and start the auxillary services
    make_target ensure-docker-network
    make_target ensure-configuration
    make_target start-svc

    # Start Brane (using the dev call)
    make_target start-brn-dev

# Stops the instance (built in a container)
elif [[ "$target" == "stop-instance-safe" ]]; then
    # Simply call the normal dev one
    make_target stop-instance-dev



### TESTING ###
# Makes the tests and runs them
elif [[ "$target" == "test" ]]; then
    # Simply run cargo
    exec_step cargo test

# Makes the files and runs the linter (clippy)
elif [[ "$target" == "linter" ]]; then
    # Simply run cargo
    exec_step cargo clippy -- -D warnings



### OTHER ###
# Unrecognized target
else
    echo "Unrecognized target '$target'." >&2
    exit 1

fi
