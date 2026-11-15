# Changelog

All notable changes to `quickrep` are documented in this file. Versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html), and entries are generated
from [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) with
[git-cliff](https://git-cliff.org).

## [1.1.0](https://github.com/kodzonko/quick-repertoire/compare/v1.0.0...v1.1.0) - 2026-04-03

### Features

- **helios:** Add Helios cinema support ([#18](https://github.com/kodzonko/quick-repertoire/pull/18)) ([ce34aa8](https://github.com/kodzonko/quick-repertoire/commit/ce34aa8384d5b3e376696d3750dae3796920442d))
- **multikino:** Add Multikino support ([#19](https://github.com/kodzonko/quick-repertoire/pull/19)) ([1e7ba81](https://github.com/kodzonko/quick-repertoire/commit/1e7ba81bfa63608c9c6710dd9ef300acaee0846f))

### CI

- Remove nightly toolchain and enforce stable-only builds ([eb6c788](https://github.com/kodzonko/quick-repertoire/commit/eb6c788ba9ccf0c90de6ffb0de00e4028b444539))
- Scope gh commands to GITHUB_REPOSITORY and remove branch coverage flag ([2729b33](https://github.com/kodzonko/quick-repertoire/commit/2729b3343e265427497a445a856c51905fb29a1c))

### Known Issues

- The 1.1.0 binaries report `quickrep 1.0.0` from `quickrep --version`, because `Cargo.toml` was not bumped for this release.

## [1.0.0](https://github.com/kodzonko/quick-repertoire/releases/tag/v1.0.0) - 2026-04-02

First release of `quickrep`, the Rust rewrite of the earlier Python prototype
(`cinema-repertoire-analyzer`), which was never released. Changes before the rewrite are not listed.

### Features

- **app:** **BREAKING:** Rewrite cli application in rust ([6894543](https://github.com/kodzonko/quick-repertoire/commit/68945431cc7c785f335b01974ab895f6762f0015))
- **config:** Store config and db near executable ([5067ef6](https://github.com/kodzonko/quick-repertoire/commit/5067ef68da2e3912649ef8d19221dc4df64d6096))
- **core:** **BREAKING:** Improve resilience and rename binary ([86a0676](https://github.com/kodzonko/quick-repertoire/commit/86a0676790dda81617b37abbb456add9c9fe7be1))
- **cinema-city:** **BREAKING:** Link bookable showtimes in output ([fa09744](https://github.com/kodzonko/quick-repertoire/commit/fa09744fc5bf08eb1f0902c1c9d7eff643a006d4))
- **tmdb:** Improve matching with Cinema City metadata ([b94ab4b](https://github.com/kodzonko/quick-repertoire/commit/b94ab4b29059af763c5295cb9809f29a3ed238aa))

### Bug Fixes

- **output:** Preserve osc-8 links in table wrapping ([ff7a028](https://github.com/kodzonko/quick-repertoire/commit/ff7a0287984d1333f87850a79e6a8f029d19eb5e))
- Ignore stale schedule rows and parse more dates ([a59ec09](https://github.com/kodzonko/quick-repertoire/commit/a59ec09a798ba1be2a5a2097ebbff06759462702))

### Performance

- **api:** Parallelize cinema city and tmdb lookups ([6004f73](https://github.com/kodzonko/quick-repertoire/commit/6004f73da04268961e77d7bbdc51ed4309f9c662))

### Refactoring

- **BREAKING:** Rename crate to quick-repertoire ([7983ad4](https://github.com/kodzonko/quick-repertoire/commit/7983ad4a080a201a3317ec1a59f7323c82ee8d8d))

### Documentation

- Remove demo and generated repo images ([2ad48f9](https://github.com/kodzonko/quick-repertoire/commit/2ad48f9afc60c88bd3676407b3b48c4c1781581d))

### Dependencies

- Bump actions/checkout from 4 to 6 ([#16](https://github.com/kodzonko/quick-repertoire/pull/16)) ([f2d93df](https://github.com/kodzonko/quick-repertoire/commit/f2d93df394691489d5ffdd12c62f707497f36f39))
- Bump actions/upload-artifact from 6 to 7 ([#17](https://github.com/kodzonko/quick-repertoire/pull/17)) ([9cff623](https://github.com/kodzonko/quick-repertoire/commit/9cff62383b4355f503d95132275a7bbcb0ffd489))

### CI

- **workflows:** Migrate macos runners to version 15 ([07c104e](https://github.com/kodzonko/quick-repertoire/commit/07c104e8d18000a5bce336e7d7123caf3ad048c6))
- Rename release binary to quickrep and add consistency test ([a7ed4b7](https://github.com/kodzonko/quick-repertoire/commit/a7ed4b7b0b4533e1b71f12bf5b77d094d17fc423))

### Miscellaneous

- Remove repository tooling and workflow configuration ([0e4bb17](https://github.com/kodzonko/quick-repertoire/commit/0e4bb177d8e0977bb2524d0616f769444dff2f09))

