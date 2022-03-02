##########
## META ##
##########

.PHONY: default all clean cli branelet branelet-safe instance instance-dev instance-safe \
		build-image images images-dev \
		ensure-docker-network ensure-configuration \
		start-instance stop-instance start-instance-dev stop-instance-dev start-instance-safe stop-instance-safe \
		start-svc stop-svc start-brn stop-brn start-brn-dev stop-brn-dev

default: cli

all: cli branelet branelet-safe instance instance-dev

clean:
	rm -rf ./target
clean_openssl:
	rm -rf ./contrib/deps/openssl



##############
## BINARIES ##
##############

cli:
	cargo build --release --package brane-cli
	@echo "Compiled the Command-Line Interface (CLI) to 'target/release/brane'"

branelet:
	rustup target add x86_64-unknown-linux-musl
	cargo build --release --package brane-let --target x86_64-unknown-linux-musl
	@echo "Compiled package initialization binary to 'target/x86_64-unknown-linux-musl/release/branelet'"

branelet-safe: build-image-dev
	docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(shell pwd):/build" brane-build-dev "build_branelet"
	@echo "Removing root ownership from target folder (might require sudo password)"
	sudo chown -R $(shell echo "$$USER"):$(shell echo "$$USER") ./target
	@echo "Compiled package initialization binary to 'target/containers/target/release/branelet'"



############
## IMAGES ##
############

%-image: Dockerfile.%
	docker build --load -t brane-$(subst -image,,$@) -f $< .
%-image-dev: Dockerfile_dev.%
	docker build --load -t brane-$(subst -image-dev,,$@)-dev -f $< .

images: \
	api-image \
	clb-image \
	drv-image \
	job-image \
	log-image \
	plr-image
images-dev: \
	api-image-dev \
	clb-image-dev \
	drv-image-dev \
	job-image-dev \
	log-image-dev \
	plr-image-dev



##############
## INSTANCE ##
##############

instance: images
	@echo "Compiled Brane instance (as Docker images)"

start-instance: instance \
	ensure-docker-network \
	ensure-configuration \
	start-svc \
	start-brn

stop-instance: \
	stop-brn \
	stop-svc

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



##################
## DEV INSTANCE ##
##################

# Build a container-side version of OpenSSL against musl
# We need this to build the ring package (since it relies on a valid OpenSSL target)
contrib/deps/openssl/lib/libssl.a: ssl-image-dev
	docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(shell pwd):/build" brane-ssl-dev "build_openssl"
	@echo "Removing root ownership from contrib folder (might require sudo password)"
	sudo chown -R $(shell echo "$$USER"):$(shell echo "$$USER") ./contrib
	@echo "Compiled OpenSSL library to 'contrib/deps/openssl/'"

instance-dev: ./contrib/deps/openssl/lib/libssl.a
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
	@echo "Compiled Brane instance to 'target/containers/target/release/'"

start-instance-dev: instance-dev \
	ensure-docker-network \
	ensure-configuration \
	start-svc \
	start-brn-dev

stop-instance-dev: \
	stop-brn-dev \
	stop-svc

start-brn-dev:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml up -d

restart-brn-dev:
	docker restart brane-api & \
	docker restart brane-clb & \
	docker restart brane-drv & \
	docker restart brane-job & \
	docker restart brane-log & \
	docker restart brane-plr & \
	wait

stop-brn-dev:
	COMPOSE_IGNORE_ORPHANS=1 docker-compose -p brane -f docker-compose-brn-dev.yml down



######################################################
## UNSAFE DEV INSTANCE (BUILD OUTSIDE OF CONTAINER) ##
######################################################

instance-safe: build-image-dev images-dev
	docker run --attach STDIN --attach STDOUT --attach STDERR --rm -v "$(shell pwd):/build" brane-build-dev "build_brane"
	@echo "Removing root ownership from target folder (might require sudo password)"
	sudo chown -R $(shell echo "$$USER"):$(shell echo "$$USER") ./target
	@echo "Compiled Brane instance to 'target/containers/target/release/'"

start-instance-safe: instance-safe \
	ensure-docker-network \
	ensure-configuration \
	start-svc \
	start-brn-dev

stop-instance-safe: stop-instance-dev

#######
