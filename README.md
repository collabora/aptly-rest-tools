# aptly-rest-tools

Rust Aptly REST api tooling.

This workspace consists of:
* apt2aptly - To mirror an existing apt repository in aptly.
* aptly-latest-snapshots - A simple REST API that simply returns the latest snapshot published for a dist.
* aptly-rest-mock - A mock crate for the aptly REST apis.
* aptly-rest - A helper crate for using the aptly REST api and some misc package handling helpers.
* aptlyctl - A thin wrapper around the REST API .
* obs2aptly - A tool to sync from a collections of package as produced by OBS into aptly.
* sync2aptly - A lib with sync functionality
