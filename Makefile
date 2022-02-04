build: build-binaries build-services

# TIM #
.PHONY: build clean build-binaries build-cli build-let build-services build-api-image build-clb-image \
	build-drv-image build-job-image build-log-image build-plr-image start-instance \
	stop-instance ensure-docker-images ensure-docker-network ensure-configuration start-svc stop-svc \
	start-brn stop-brn build-services-dev build-api-image-dev build-clb-image-dev \
	build-drv-image-dev build-job-image-dev build-log-image-dev build-plr-image-dev start-instance-dev \
	stop-instance-dev ensure-docker-images-dev start-brn-dev stop-brn-dev build-image-dev

clean:
	rm -rf ./target
#######

##############
## BINARIES ##
##############

build-binaries: \
	build-cli \
	build-let

build-cli:
	cargo build --release --package brane-cli

build-let:
	rustup target add x86_64-unknown-linux-musl
	TARGET_CC="x86_64-linux-musl-cc" \
	TARGET_CXX="x86_64-linux-musl-c++" \
	cargo build --release --package brane-let --target x86_64-unknown-linux-musl

build-let-containerized: build-bld-image-dev
	docker run --attach STDOUT --attach STDERR -v "$(shell pwd):/build" build-image-dev "build_branelet" --rm
	echo "Compiled branelet to target/containers/target"

##############
## SERVICES ##
##############

build-services: \
	build-api-image \
	build-clb-image \
	build-drv-image \
	build-job-image \
	build-log-image \
	build-plr-image

build-api-image:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-api -f Dockerfile.api .

build-clb-image:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-clb -f Dockerfile.clb .

build-drv-image:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-drv -f Dockerfile.drv .

build-job-image:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-job -f Dockerfile.job .

build-log-image:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-log -f Dockerfile.log .

build-plr-image:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-plr -f Dockerfile.plr .



# TIM #
##############
## DEVBUILD ##
##############

build-services-dev: \
	build-api-image-dev \
	build-clb-image-dev \
	build-drv-image-dev \
	build-job-image-dev \
	build-log-image-dev \
	build-plr-image-dev

build-bld-image-dev:
	docker build --load -t build-image-dev -f Dockerfile_dev.bld .

build-api-image-dev:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-api-dev -f Dockerfile_dev.api .

build-clb-image-dev:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-clb-dev -f Dockerfile_dev.clb .

build-drv-image-dev:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-drv-dev -f Dockerfile_dev.drv .

build-job-image-dev:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-job-dev -f Dockerfile_dev.job .

build-log-image-dev:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-log-dev -f Dockerfile_dev.log .

build-plr-image-dev:
	docker build --load -t ghcr.io/onnovalkering/brane/brane-plr-dev -f Dockerfile_dev.plr .
#######

##############
## INSTANCE ##
##############

start-instance: \
	ensure-docker-images \
	ensure-docker-network \
	ensure-configuration \
	start-svc \
	start-brn

stop-instance: \
	stop-brn \
	stop-svc

ensure-docker-images:
	if [ -z "${BRANE_VERSION}" ]; then \
		make build-services; \
	fi;

ensure-docker-network:
	if [ ! -n "$(shell docker network ls -f name=brane | grep brane)" ]; then \
		docker network create brane; \
	fi;

ensure-configuration:
	touch infra.yml && \
	touch secrets.yml

start-svc:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml up -d
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml rm -f

stop-svc:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-svc.yml down

start-brn:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml up -d

stop-brn:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn.yml down


# TIM #
##################
## DEV INSTANCE ##
##################

start-instance-dev: \
	ensure-docker-images-dev \
	ensure-docker-network \
	ensure-configuration \
	build-brane \
	start-svc \
	start-brn-dev

restart-instance-dev: \
	build-brane \
	restart-brn-dev

stop-instance-dev: \
	stop-brn-dev \
	stop-svc

ensure-docker-images-dev:
	if [ -z "${BRANE_VERSION}" ]; then \
		make build-services-dev; \
	fi;

# Build a container-side version of OpenSSL against musl
# We need this to build the ring package (since it relies on a valid OpenSSL target)
contrib/deps/openssl/lib/libssl.a:
	docker build --load -t build-ssl-image-dev -f Dockerfile_dev.ssl .
	docker run --rm --attach STDOUT --attach STDERR -v "$(shell pwd):/build" build-ssl-image-dev "build_openssl"
	echo "Require root permissions to revert root permissions on musl directory:"
	sudo chown -R $(shell echo "$$USER"):$(shell echo "$$USER") ./contrib/deps/openssl

build-brane: ./contrib/deps/openssl/lib/libssl.a
	rustup target add x86_64-unknown-linux-musl
	OPENSSL_DIR="$(shell pwd)/contrib/deps/openssl" \
	OPENSSL_LIB_DIR="$(shell pwd)/contrib/deps/openssl/lib" \
	cargo build \
		--release \
		--target-dir "./target/containers/target" \
		--target x86_64-unknown-linux-musl \
		--package "brane-api" \
		--package "brane-clb" \
		--package "brane-drv" \
		--package "brane-job" \
		--package "brane-log" \
		--package "brane-plr"
	mkdir -p ./target/containers/target/release/
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-api ./target/containers/target/release/
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-clb ./target/containers/target/release/
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-drv ./target/containers/target/release/
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-job ./target/containers/target/release/
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-log ./target/containers/target/release/
	/bin/cp -f ./target/containers/target/x86_64-unknown-linux-musl/release/brane-plr ./target/containers/target/release/

start-brn-dev:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml up -d

restart-brn-dev:
	docker restart brane-api
	docker restart brane-clb
	docker restart brane-drv
	docker restart brane-job
	docker restart brane-log
	docker restart brane-plr

stop-brn-dev:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml down



######################################################
## UNSAFE DEV INSTANCE (BUILD OUTSIDE OF CONTAINER) ##
######################################################

start-instance-dev-containerized: \
	ensure-docker-images-dev \
	ensure-docker-network \
	ensure-configuration \
	build-brane-containerized \
	start-svc \
	start-brn-dev

build-brane-containerized: build-bld-image-dev
	docker run --attach STDOUT --attach STDERR -v "$(shell pwd):/build" build-image-dev "build_brane" --rm
	echo "Compiled Brane to target/containers/target"

#######
