# aptly-rest-tools

**aptly-rest-tools** is a collection of Rust-based tools and libraries
designed to interact with and extend the functionality of the [Aptly REST
API](https://www.aptly.info/doc/api/).

### `aptly-rest`
A foundational Rust library that provides a typed interface to the Aptly
REST API, along with utility functions for handling Debian packages and
repository structures.

### `aptly-rest-mock`
A mock server that simulates the Aptly REST API. Enables testing tools
and libraries without requiring a live Aptly instance.

### `aptlyctl`
A command-line tool for interacting with Aptly via its REST API. Wraps
common operations such as snapshot creation, repository publishing,
and cleanup into an easy-to-use CLI.

### `apt2aptly`
Mirrors an existing apt repository into an Aptly-managed
repository. Useful for importing upstream Debian/Ubuntu repositories or
third-party apt sources.

### `obs2aptly`
Synchronizes packages from an [Open Build Service
(OBS)](https://openbuildservice.org/) instance into Aptly. Designed for
teams using OBS for package builds and Aptly for distribution.

### `sync2aptly`
A Rust library providing synchronization logic between upstream sources
and Aptly repositories. Used by both `apt2aptly` and `obs2aptly`.

### `aptly-latest-snapshots`
A lightweight HTTP API that returns the latest published snapshot for a
given distribution. Intended for external systems to dynamically query
the most recent publish state.


## Installation

    cargo install --locked --git https://github.com/collabora/aptly-rest-tools aptlyctl

This will install `aptlyctl` into `~/.cargo/bin`.

## Usage

Set the token as environment variable to interact with the aptly instance.
Alternatively, it can be passed directly to `aptlyctl` with the argument `--api-token`.

    export APTLY_API_TOKEN=XXXXXXXXXXXXXXXX

### List repositories

    aptlyctl \
        -u https://repositories.apertis.org/apertis/_aptly \
        repo list

### Create repository

    aptlyctl \
        -u https://repositories.apertis.org/apertis/_aptly \
        repo create \
        --component non-free \
        --distribution apertis \
        apertis:v2024dev0:non-free/default

### Publish repository

    aptlyctl \
        -u https://repositories.apertis.org/apertis/_aptly \
        publish create repo apertis \
        --architecture=source \
        --architecture=armhf \
        --architecture=amd64 \
        --architecture=arm64 \
        --distribution=v2024dev0 \
        --skip-contents --skip-bz2 \
        apertis:v2024dev0:development/default//development \
        apertis:v2024dev0:sdk/default//sdk \
        apertis:v2024dev0:target/default//target \
        apertis:v2024dev0:non-free/default//non-free \
        --gpg-key=XXXXXXXX

### Drop repository

    aptlyctl \
        -u https://repositories.apertis.org/apertis/_aptly \
        repo drop apertis:v2024dev0:non-free/default

### Drop publish

    aptlyctl \
        -u https://repositories.apertis.org/apertis/_aptly \
        publish drop apertis v2024dev0:non-free

## Contributing

Contributions, bug reports, and feature suggestions are welcome. Please open an issue or submit a pull request with clear descriptions of the changes.

## License

This project is dual-licensed under the [Apache 2.0](./LICENSE-APACHE) and [MIT](./LICENSE-MIT) licenses.
