# upp

> batteries-included uptime monitor for your infrastructure

`upp` runs off a single binary, a configuration file (in toml) and an sqlite database (can be in memory or persisted on disk), and provides a daemon that tests configured routes while also serving a tiny API and web frontend.

it periodically makes requests to configured services, and tracks roundtrip time (or if no response was returned at all!). this data is then accessible using `upp` tiny builtin api, and can be viewed on the integrated webpage (served on service's `/`)

as an example, check out my instance on [up.alemi.dev](https://up.alemi.dev)

## goals
this aims to be super simple to use to glance at your stuff and help figuring out if there are issues. this doesn't aim to be an extensive monitoring solution for infrastructure, and while it can fit the role it isn't actively maintained against that use-case.

future features which would be nice to add are:
 * notifications on downtime
 * auto-cleanup of old samples
 * configurable frontend span
 * graph references on frontend
 * data exporters
 * full database engine to use mysql/psql
 * multi-protocol endpoint tester

## name
small but not really specific, `uprs` and `up-rs` and `up` were taken. if you have better name ideas let me know c:
