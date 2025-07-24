# upp
[![Actions Status](https://github.com/alemidev/upp/actions/workflows/test.yml/badge.svg?branch=dev)](https://github.com/alemidev/upp/actions/workflows/test.yml)
[![Actions Status](https://github.com/alemidev/upp/actions/workflows/release.yml/badge.svg)](https://github.com/alemidev/upp/actions/workflows/release.yml)
[![Crates.io Version](https://img.shields.io/crates/v/upp)](https://crates.io/crates/upp)
[![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/upp)](https://crates.io/crates/upp)
[![GitHub last commit](https://img.shields.io/github/last-commit/alemidev/upp)](https://github.com/alemidev/upp/commits/dev/)
[![GitHub commits since tagged version](https://img.shields.io/github/commits-since/alemidev/upp/v0.2.2)](https://github.com/alemidev/upp/releases/tag/v0.2.2)

> batteries-included uptime monitor for your infrastructure

`upp` runs off a single binary, a configuration file (in toml) and an sqlite database (can be in memory or persisted on disk), and provides a daemon that tests configured routes while also serving a tiny API and web frontend.

it periodically makes requests to configured services, and tracks roundtrip time (or if no response was returned at all!). this data is then accessible using `upp` tiny builtin api, and can be viewed on the integrated webpage (served on service's `/`)

as an example, check out my instance on [up.alemi.dev](https://up.alemi.dev)

## goals
this aims to be super simple to use to glance at your stuff and help figuring out if there are issues. this doesn't aim to be an extensive monitoring solution for infrastructure, and while it can fit the role it isn't actively maintained against that use-case.

future features which would be nice to add are:
 * notifications on downtime
 * auto-cleanup of old samples
 * data exporters
 * full database engine to use mysql/psql
 * multi-protocol endpoint tester
