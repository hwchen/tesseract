# Tesseract

[![Build Status](https://travis-ci.org/tesseract-olap/tesseract.svg?branch=master)](https://travis-ci.org/tesseract-olap/tesseract)

More documentation is coming soon!

## Setup

### Caching

When using JWT authentication for requests, it is recommended to use Redis as the request cache instead of Nginx. The reason is that Tesseract is able to exclude the JWT token from the URL, which helps increase cache hits. To set up Redis, follow [this](https://www.digitalocean.com/community/tutorials/how-to-install-and-secure-redis-on-ubuntu-18-04) guide.

Once Redis is installed, set the `TESSERACT_REDIS_URL` environment variable to the address and port where Redis is running and restart Tesseract.

**IMPORTANT:** You can customize the cache by setting its max memory limit and eviction policy. Add the following lines to `/etc/redis/redis.conf`:

```
maxmemory 10gb
maxmemory-policy allkeys-lru
```

For more information, refer to [this](https://redis.io/topics/lru-cache) guide.

## For Users

### Getting started

Tesseract serves an API which allows users to drilldown, cut, filter, and otherwise examine a [cube](https://en.wikipedia.org/wiki/OLAP_cube) of data.

The logical construct of the "cube" allows for powerful, flexible, and fast data analysis while keeping the data in the most efficient physical format (in-db).

1) Get `tesseract`: See installation instructions below.
2) Get data into "cube" format: [star schema](https://en.wikipedia.org/wiki/Star_schema)-like is optimal.
3) Write a schema, which shows how the logical representation of the cube maps to the data in the database.
4) Set options as environment variables and/or CLI flags. See instructions below.
5) (Optional) Set up a process monitor like systemd.
6) Run tesseract! For some examples of CLI invocations, see the [justfile](https://github.com/hwchen/tesseract/blob/master/justfile). 

If you installed using Homebrew on macOS, the binary is automatically moved to your `usr/local/bin` folder and is called `tesseract-olap`.

Note: as of v0.13.0 the binary is called `tesseract-olap` on both linux and osx.

### Installation

#### macOS

Using [Homebrew](https://brew.sh/):

```
brew tap tesseract-olap/tesseract https://github.com/tesseract-olap/tesseract.git
brew install tesseract-olap
```

#### Linux

For now, just `wget` and `dpkg -i`. In the future, a PPA may be set up.

```
wget https://github.com/tesseract-olap/tesseract/releases/latest/download/tesseract-olap.deb
dpkg -i tesseract-olap.deb
```

You can then run the binary `tesseract-olap`.

Note that a systemd `.service` is also installed. You will probably need to modify the defaults, and you can do so at the install script prompt. To start the tesseract service, use `systemctl start tesseract-olap`.

#### Docker

Clone this repository and build the docker image using the command `docker build -t tesseract:latest .`

Then you can run a container using the command
```
docker run [-e ENV_VAR=value] tesseract:latest
```

Don't forget to set the needed [environment variables](#environment-variables). The container will expose the server in port 7777. You can then bind the port to the host machine or connect another container.

### Environment Variables
- `TESSERACT_DATABASE_URL`: required, is the address of the database; make sure to include the user, password, and database name.
- `TESSERACT_DEBUG`: boolean, `true` is a flag to enable more verbose logging output to help the debugging process while testing.
- `TESSERACT_FLUSH_SECRET`: optional, but required for flush; is the secret key for the flush endpoint.
- `TESSERACT_LOGIC_LAYER_CONFIG_FILEPATH`: optional, should point to the location on path for the logic layer configuration.
- `TESSERACT_SCHEMA_FILEPATH`: required, should point to the location on disk for the tesseract schema file.
- `TESSERACT_STREAMING_RESPONSE`: `boolean, true` streams rows/blocks as database streaming allows.

- `RUST_LOG`: optional, sets logging level. I generally set to `info`.

#### Docker Example

In order to run a container with the highest verbose level of debugging, we would do it like this:

```
docker run -p 7777:7777 -v ~/CODE/dw-localenv-tesseract:/dw-localenv-tesseract -e TESSERACT_DATABASE_URL='clickhouse://default:@host.docker.internal:9000/default' -e TESSERACT_SCHEMA_FILEPATH='/dw-localenv-tesseract/schema.xml' -e TESSERACT_DEBUG=true -e RUST_LOG=debug --name=tesseract-local tesseract:latest
```

Note that the `7777` port is published to the host, we pass the repository with the schema file (`dw-localenv-tesseract`) as a volume to the container and we connect to another container with a ClickHouse Server running. To access the host's `localhost` address from the container we must use:

* **Windows**: `host.docker.internal`
* **Mac OS**: `host.docker.internal` or `docker.for.mac.localhost`
* **Linux**: `172.17.0.1`

### API documentation

For more details on the api, please check the server [readme](https://github.com/hwchen/tesseract/blob/master/tesseract-server/README.md). This will soon be updated and easier to follow on a separate documentation site.

For more details on the logic layer api, check [here](https://github.com/hwchen/tesseract/blob/master/tesseract-server/src/logic_layer/README.md). This will also be updated and easier to follow on a separate documentation site.

## For Developers

### Dev Environment

To make life easier, the development environment uses:
- [just](https://github.com/casey/just) (a command runner)
- [watchexec](https://github.com/watchexec/watchexec) (executes command on file changes)

You can install them via `cargo` (or see their respectively webpages):
```
cargo install just
cargo install watchexec
```

Make sure your `~/.cargo/bin` is in your `PATH`.

We also recommended using something like [direnv](https://github.com/direnv/direnv) to manage environment variables.

### Dev commands
From the root of the repository folder:
- `just serve`: serves from debug build, using env vars for options
- `just deploy {{target}}`: builds `--release` and will scp to target of your choice
- `just check`: an alias for `watchexec cargo check`

## Contributors
Tesseract was originally created by @hwchen and is currently maintained by @MarcioPorto and @jspeis of @Datawheel.

## License

MIT license (LICENSE.md or http://opensource.org/licenses/MIT)

