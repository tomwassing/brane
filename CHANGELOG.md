# Changelog

All notable changes to the Brane framework will be documented in this file.

## [0.6.0] - 2022-03-02
### Added
- Garbage collection to custom Heap backend.

### Changed
- Branelet, brane-drv, brane-clb and brane-job to allow for much more feedback to reported to the user (when the job returns non-zero exit codes, when branelet fails to launch, ...).
- Opcodes to be an enum, allowing more streamlined conversion to names and changing opcode numbers.
- The 'build' and 'import' subcommands to be much more verbose in errors (especially in referenced files in container.yml).
- The 'build' and 'import' to use '--workdir' instead of '--context', also changing its behaviour to a more intuitive version.
- The organisation of utils.rs in brane-cli (includes commonly used functions from package.rs + its own error enum).
- The Makefile to a Bash script (`./make.sh`) with much of the same functionality, except that is has better rebuild checking.
- Code structure of bytecode.rs, frames.rs, objects.rs and stack.rs in brane-bvm.
- Handles in the Heap backend completely, so they can now be used without being passed a Heap object.
- The PackageInfo / ContainerInfo files to now have OpenAPI document / ContainerInfo as user interface, PackageInfo as general backend metadata and LocalContainerInfo as image-local file for branelet.
- The directory utilities to not automatically create directories anymore (we have separate functions for that now).
- Some crates to use clap again, as we finally found the issue (missing 'env' feature).

### Fixed
- Tests not compiling.
- OP_PARALLEL being disabled; it's now working again as expected.
- The Brane executable making files instead of directories when making standard config directories.
- Docker not refreshing images with the same version after building them or pushing them.

## [0.5.0] - 2022-02-10
### Added
- '/health' path in brane-api to follow tutorial more closely.
- Names for Brane service containers to allow more easy interaction.
- A lot of additional error catching and reporting across the entire project (but still a lot to do).
- VM now properly returning internal errors to the user (when running either locally or remotely).

### Changed
- The project is now being worked on by a new owner (Tim, pleased to meet you).
- Version can now be omitted when pushing, defaulting to the latest version instead.
- Version can now be specified more intuitively when removing a package.
- Streamlined naming of 'ecu' packages; all naming of them as 'code' has been changed to 'ecu'.
- Compiling Brane for development purposes. It's now possible to cross-compile locally on a shared Docker partition (saving a lot of time on macOS), meaning that it doesn't have to rebuild from scratch every time the containers are launched.
- Brane-bvm's heap backend to a custom one, because the old one did not play well with threads and parallelism.
- OP_PARALLEL to be temporarily disabled due to new heap.

### Fixed
- Clamp not compiling anymore; using StructOpt in most cases instead
- Branelet not being able to run OpenAPI package properly due to incorrectly replacing URL values (expected '{' and '}', but actually got '%7B' and '%7D').
- Containers not being able to reach each other due to incorrect IPs (most assumed '127.0.0.1', but this only worked for thing outside of Docker).
- Brane-job causing the Docker engine to connect to '127.0.0.1:5000/127.0.0.1:5000'; removed one of the two hostnames (specifically, the one in brane-job itself).
- Network 'kind' not being found; changed it to 'brane' in the default infra.yml, as this is also the network name used in the Docker Compose files.
- The type of an Array not being resolved properly, causing to error down the line due to incompatible types (while they in fact are).
- VM crashing whenever the job returned no output.

## [0.4.1] - 2021-08-16
### Fixed
- Disable debug logging from within WaitUntil future.
- Always use offset of at least 1 for nested call frames.
- Uniformly handle local and remote jobs.
- Propagate debug, stdout, and stderr output from driver to client.
- Construct package index from graphql endpoint.
- Missing scylla address in docker-compose-brn.yml

## [0.4.0] - 2021-08-11
### Added
- BraneScript, an alternative to Bakery with more a C-like syntax.
- GraphQL endpoint for querying application event logs, including subscriptions.
- Initial support for proxies and bridge functions: `brane-net`.
- Allow checkout folder name to be different than 'brane' (by [romnn](https://github.com/romnn)).
- Automated (daily) audits and multi-platform builds using GitHub actions.
- Optional flag to keep temporary package build files.
- Automatically add `token` and `server` arguments for OAS functions. 

### Changed
- Use seperate service for scheduling functions: `brane-job`.
- Use seperate library for OpenAPI support: `brane-oas`.
- REPL is now based on the `rustyline` library.
- Use gRPC for drivers (REPL and Jupyter kernel).
- Switched from Cassandra to ScyllaDB, and removed PostgreSQL dependency.
- DSL implementation is based on parser combinatorics, with `nom`.
- Switched from `actix` to `warp` as the framework for `brane-api`.

### Fixed
- Minor fixes for the word count quickstart.
- Correctly convert between DSL values and specification values.

## [0.3.0] - 2021-03-03
### Added
- Generate convenience function for CWL workflows with a single required parameter.
- `run` command to run DSL script from files. 
- `import` command to import packages from a GitHub repository.
- JupyterLab-based registry viewer.

## Changed
- The `import` DSL statement accepts multiple packages on the same line.
- Optional properties do not have to be specified while creating an object in the DSL.
- Cell output shows progress indicator and has time statistics.

## [0.2.0] - 2020-12-15
### Added
- Contributing guide, code of conduct, and issue templates (bug & feature).
- LOFAR demonstration
- Session attach/detach mechanism in JupyterLab.
- Custom renderers in JupyterLab.

### Changed
- Docker, HPC (Xenon), and Kubernetes runners are now configurable.
- Removing a package also removes it locally from Docker.
- CWL packages are now also locally testable.

### Fixed
- Various bug fixes and improvements.
- Allow pointers when creating arrays and objects in Bakery.

## [0.1.0] - 2020-06-04
### Added
- Initial implementation.
