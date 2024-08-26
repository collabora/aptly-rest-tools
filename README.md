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

### Contributing

Contributions, bug reports, and feature suggestions are welcome. Please open an issue or submit a pull request with clear descriptions of the changes.

### License

This project is dual-licensed under the [Apache 2.0](./LICENSE-APACHE) and [MIT](./LICENSE-MIT) licenses.
