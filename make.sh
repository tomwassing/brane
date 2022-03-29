#!/bin/bash
# MAKE.sh
#   by Lut99
#
# Created:
#   03 Mar 2022, 17:03:04
# Last edited:
#   28 Mar 2022, 17:47:39
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





# Helper function that checks if any of the given files have changed
# Each arguments passed is interpreted as a file
# Returns true if they have changed, or false otherwise
needs_rebuilding() {
    # Make the cache if it doesn't exist
    mkdir -p ./target/make_cache/

    # Loop through the dependencies to check them
    local different=0
    for dep in "$@"; do
        # # If it's a directory, do the dependencies by recursion
        # if [[ -d "$dep" ]]; then
        #     local deps=$(ls $dep)
        #     different=$(needs_rebuilding $deps)
        #     continue
        # fi

        # Create the target directory
        mkdir -p ./target/make_cache/$(dirname "$dep")

        # Compute the hash and check if its changed
        local hash=$(sha256sum "$dep" | cut -d " " -f 1)
        if [[ -f "./target/make_cache/$dep" && "$hash" == $(cat "./target/make_cache/$dep") ]]; then
            different=$different
        else
            different=1
        fi
    done

    # Return if we found a changed file
    echo $different
}

# Helper function that logs the current hash of the given file in the build cache
update_cache() {
    # Make sure it is given
    if [[ $# -ne 1 ]]; then
        echo "update_cache(): no file to update given" >&2
        exit -1
    fi

    # Switch on the type
    local hash=""
    local name="$1"
    if [[ "$1" =~ ^image://.* ]]; then
        # Get the hash from Docker
        name="${image:8}"
        hash=$(docker images --no-trunc --quiet "$name")
        name="_image-$name"
    else
        # Compute the hash from disk
        hash=$(sha256sum "$1" | cut -d " " -f 1)
    fi

    # Make its directories in the cache & write the hash
    mkdir -p ./target/make_cache/$(dirname "$1")
    echo "$hash" > "./target/make_cache/$1"
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
    ./make.sh instance || exit $?
    ./make.sh branelet || exit $?
    ./make.sh cli || exit $?

# Clean the standard build folder
elif [[ "$target" == "clean" ]]; then
    # Remove the target folder
    echo " > rm -rf ./target"
    rm -rf ./target || exit $?

# Clean the OpenSSL build
elif [[ "$target" == "clean_openssl" ]]; then
    # Remove the openssl folder
    echo " > rm -rf ./contrib/deps/openssl"
    rm -rf ./contrib/deps/openssl || exit $?



### BINARIES ###
# Build the command-line interface 
elif [[ "$target" == "cli" ]]; then
    # Use cargo to build the project; it manages dependencies and junk
    echo " > cargo build --release --package brane-cli"
    cargo build --release --package brane-cli || exit $?

    # Done
    echo "Compiled executeable \"brane\" to './target/release/brane'"

# Build the branelet executable by cross-compiling
elif [[ "$target" == "branelet" ]]; then
    # We let cargo sort out dependencies
    echo " > rustup target add x86_64-unknown-linux-musl"
    rustup target add x86_64-unknown-linux-musl || exit $?
    echo " > cargo build --release --package brane-let --target x86_64-unknown-linux-musl"
	cargo build --release --package brane-let --target x86_64-unknown-linux-musl || exit $?

    # Copy the resulting executable to the output branelet
    echo " > mkdir -p ./target/containers/target/release/"
    mkdir -p ./target/containers/target/release/ || exit $?
    echo " > cp ./target/x86_64-unknown-linux-musl/release/branelet ./target/containers/target/release/"
    cp ./target/x86_64-unknown-linux-musl/release/branelet ./target/containers/target/release/ || exit $?

    # Done
	echo "Compiled package initialization binary \"branelet\" to './target/containers/target/release/branelet'"

# Build the branelet executable by containerization
elif [[ "$target" == "branelet-safe" ]]; then
    # Dependencies: build the build image first
    ./make.sh build-image-dev || exit $?

    # Otherwise, continue the build as normal (by running it in a container)
    echo " > docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v '$(pwd):/build' brane-build-dev 'build_branelet'"
    docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(pwd):/build" brane-build-dev "build_branelet" || exit $?

    # Restore permissions
	echo "Removing root ownership from target folder (might require sudo password)"
	echo " > sudo chown -R '$USER':'$USER' ./target"
	sudo chown -R "$USER":"$USER" ./target || exit $?

    # Done
	echo "Compiled package initialization binary \"branelet\" to './target/containers/target/release/branelet'"



### IMAGES ###
# Build the build image
elif [[ "$target" == "build-image-dev" ]]; then
    # Then, call upon Docker to build it (it tackles caches)
    echo " > docker build --load -t brane-build-dev -f Dockerfile_dev.build ."
    docker build --load -t brane-build-dev -f Dockerfile_dev.build . || exit $?

    # Done
    echo "Built build image to Docker Image 'brane-build-dev'"

# Build the regular images
elif [[ "${target: -6}" == "-image" ]]; then
    # Get the name of the image
    image_name="${target%-image}"

    # Call upon Docker to build it (building in release as normal does not use any caching other than the caching of the image itself, sadly)
    echo " > docker build --load -t brane-$image_name -f Dockerfile.$image_name ."
    docker build --load -t "brane-$image_name" -f Dockerfile.$image_name . || exit $?

    # Done
    echo "Built $image_name image to Docker Image 'brane-$image_name'"

# Build the dev version of the images
elif [[ "${target: -10}" == "-image-dev" ]]; then
    # Get the name of the image
    image_name="${target%-image-dev}"

    # Call upon Docker to build it (we let it deal with caching)
    echo " > docker build --load -t brane-$image_name-dev -f Dockerfile_dev.$image_name ."
    docker build --load -t "brane-$image_name-dev" -f Dockerfile_dev.$image_name . || exit $?

    # Done
    echo "Built $image_name development image to Docker Image 'brane-$image_name-dev'"

# Target that bundles all the normal images together
elif [[ "$target" == "images" ]]; then
    # Simply build the images
    ./make.sh api-image || exit $?
    ./make.sh clb-image || exit $?
    ./make.sh drv-image || exit $?
    ./make.sh job-image || exit $?
    ./make.sh log-image || exit $?
    ./make.sh plr-image || exit $?

# Target that bundles all the development images together
elif [[ "$target" == "images-dev" ]]; then
    # Simply build the images
    ./make.sh api-image-dev || exit $?
    ./make.sh clb-image-dev || exit $?
    ./make.sh drv-image-dev || exit $?
    ./make.sh job-image-dev || exit $?
    ./make.sh log-image-dev || exit $?
    ./make.sh plr-image-dev || exit $?



### INSTANCE HELPERS ###
# Makes sure the docker network for Brane is up and running
elif [[ "$target" == "ensure-docker-network" ]]; then
    # Only add it if it doesn't exist already
    if [ ! -n "$(docker network ls -f name=brane | grep brane)" ]; then
        echo " > docker network create brane"
		docker network create brane || exit $?
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
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml up -d"
    COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml up -d || exit $?
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml rm -f"
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml rm -f || exit $?

    # Done
    echo "Started auxillary Brane services"

# Stops the auxillary services
elif [[ "$target" == "stop-svc" ]]; then
    # Use Docker compose again
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml down"
    COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml down || exit $?

    # Done
    echo "Stopped auxillary Brane services"



### INSTANCE ###
# Builds the instance (which is just building the normal images)
elif [[ "$target" == "instance" ]]; then
    ./make.sh images || exit $?
    echo "Built Brane instance as Docker images"

# Starts the Brane services (the normal images)
elif [[ "$target" == "start-brn" ]]; then
    # Use Docker compose to start them
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml up -d"
    COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml up -d || exit $?

    # Done
    echo "Started Brane services"

# Stops the Brane services (the normal images)
elif [[ "$target" == "stop-brn" ]]; then
    # Use Docker compose again
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml down"
    COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml down || exit $?

    # Done
    echo "Stopped Brane services"

# Starts the instance (from the normal images)
elif [[ "$target" == "start-instance" ]]; then
    # Build the instance first
    ./make.sh instance || exit $?

    # Ensure that everything is in order and start the auxillary services
    ./make.sh ensure-docker-network || exit $?
    ./make.sh ensure-configuration || exit $?
    ./make.sh start-svc || exit $?

    # Start Brane
    ./make.sh start-brn || exit $?

# Stops the instance (from the normal images)
elif [[ "$target" == "stop-instance" ]]; then
    # Stop Brane
    ./make.sh stop-brn || exit $?

    # Stop the auxillary services
    ./make.sh stop-svc || exit $?



### DEVELOPMENT INSTANCE ###
# Build OpenSSL
elif [[ "$target" == "openssl" ]]; then
    # Prepare the build image for the SSL
    ./make.sh ssl-image-dev || exit $?

    # Compile the OpenSSL library
    echo " > docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v \"$(pwd):/build\" brane-ssl-dev \"build_openssl\""
    docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(pwd):/build" brane-ssl-dev "build_openssl" || exit $?

    # Restore the permissions
	echo "Removing root ownership from target folder (might require sudo password)"
    echo " > sudo chown -R "$USER":"$USER" ./target"
	sudo chown -R "$USER":"$USER" ./target || exit $?

    # Done
	echo "Compiled OpenSSL library to 'target/openssl/'"

# Build the instance (by cross-compiling)
elif [[ "$target" == "instance-dev" ]]; then
    # Build openssl only if any of the files is missing
    for target in "${OPENSSL_TARGETS[@]}"; do
        if [[ ! -f "$target" ]]; then
            ./make.sh openssl || exit $?
            break
        fi
    done

    # Build the instance images
    ./make.sh images-dev || exit $?

    # Prepare the cross-compilation target
    echo " > rustup target add x86_64-unknown-linux-musl"
    rustup target add x86_64-unknown-linux-musl || exit $?

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
		--package brane-plr
    
    # Copy the results to the correct location
    echo " > mkdir -p ./target/containers/target/release/"
    mkdir -p ./target/containers/target/release/
    echo " > /bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-{api,clb,drv,job,log,plr} ./target/containers/target/release/"
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-{api,clb,drv,job,log,plr} ./target/containers/target/release/

    # Done!
    echo "Compiled Brane instance to 'target/containers/target/release/'"

# Starts the Brane services (cross-compiled)
elif [[ "$target" == "start-brn-dev" ]]; then
    # Use Docker compose to start them
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml up -d"
    COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml up -d || exit $?

    # Done
    echo "Started Brane services"

# Stops the Brane services (cross-compiled)
elif [[ "$target" == "stop-brn-dev" ]]; then
    # Use Docker compose again
    echo " > COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml down"
    COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml down || exit $?

    # Done
    echo "Stopped Brane services"

# Starts the instance (cross-compiled)
elif [[ "$target" == "start-instance-dev" ]]; then
    # Build the instance first
    ./make.sh instance-dev || exit $?

    # Ensure that everything is in order and start the auxillary services
    ./make.sh ensure-docker-network || exit $?
    ./make.sh ensure-configuration || exit $?
    ./make.sh start-svc || exit $?

    # Start Brane
    ./make.sh start-brn-dev || exit $?

# Stops the instance (cross-compiled)
elif [[ "$target" == "stop-instance-dev" ]]; then
    # Stop Brane
    ./make.sh stop-brn-dev || exit $?

    # Stop the auxillary services
    ./make.sh stop-svc || exit $?



### DEVELOPMENT INSTANCE CONTAINERIZED ###
# Build the instance (by containerization)
elif [[ "$target" == "instance-safe" ]]; then
    # Build the build image
    ./make.sh build-image-dev || exit $?

    # Use Docker to build it
    echo " > docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v \"$(pwd):/build\" brane-build-dev \"build_brane\""
    docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(pwd):/build" brane-build-dev "build_brane" || exit $?

    # Remove
	echo "Removing root ownership from target folder (might require sudo password)"
    echo " > sudo chown -R "$USER":"$USER" ./target"
	sudo chown -R "$USER":"$USER" ./target

    # Done
	echo "Compiled Brane instance to 'target/containers/target/release/'"

# Starts the instance (built in a container)
elif [[ "$target" == "start-instance-safe" ]]; then
    # Build the instance first
    ./make.sh instance-safe || exit $?

    # Ensure that everything is in order and start the auxillary services
    ./make.sh ensure-docker-network || exit $?
    ./make.sh ensure-configuration || exit $?
    ./make.sh start-svc || exit $?

    # Start Brane (using the dev call)
    ./make.sh start-brn-dev || exit $?

# Stops the instance (built in a container)
elif [[ "$target" == "stop-instance-safe" ]]; then
    # Simply call the normal dev one
    ./make.sh stop-instance-dev || exit $?



### TESTING ###
# Makes the tests and runs them
elif [[ "$target" == "test" ]]; then
    # Simply run cargo
    echo " > cargo test"
    cargo test || exit $?



### OTHER ###
# Unrecognized target
else
    echo "Unrecognized target '$target'." >&2
    exit 1

fi
