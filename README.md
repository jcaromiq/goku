# Goku 
[![Rust](https://github.com/jcaromiq/goku/actions/workflows/ci.yml/badge.svg)](https://github.com/jcaromiq/goku/actions/workflows/ci.yml)
[![](https://img.shields.io/crates/v/goku-bench.svg?ts=2)](https://crates.io/crates/goku-bench)

![Goku](https://raw.githubusercontent.com/jcaromiq/goku/main/assets/goku.png)

Goku is a high-performance, scalable HTTP load-testing tool designed for benchmarking and performance analysis of web services. Inspired by tools like [Drill](https://github.com/fcsonline/drill) and [Vegeta](https://github.com/tsenart/vegeta), Goku offers modern features and simplicity for engineers to simulate and analyze traffic efficiently.

## Features
* Fast and scalable HTTP load testing
* Supports structured, real-time metrics
* Detailed performance analytics
    

## Install
### Automatic download (Linux, OSX, WSL)

You can download the latest version of Goku directly to your current directory with the following command:

```bash
curl -sSL https://raw.githubusercontent.com/jcaromiq/goku/v1.1.7/scripts/install.sh | sh
```

### Using Cargo
```bash
cargo install goku-bench
goku --version
```

### Manual download

Go to the Goku's [GitHub Releases page](https://github.com/jcaromiq/goku/releases) and download the latest `.tar.gz` file that matches your system. Currently, tarballs are available for the following:

* Linux (x86_64)
* macOS (x86_64)
* Windows (x86_64)

### Source

As a requirement, you need `rust` installed:

```shell
$ cargo build --release
```

## Versioning

CLI is versioned with [SemVer v2.0.0](https://semver.org/spec/v2.0.0.html).

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md).

## Usage manual

```console
Usage: goku [OPTIONS] --target <TARGET>

Options:
  -v, --verbose                      Runs in verbose mode
  -t, --target <TARGET>              URL to be requested using an operation [default: GET] Ex. GET http://localhost:3000/
  -r, --request-body <REQUEST_BODY>  File path for the request body
  -c, --clients <CLIENTS>            Number of concurrent clients [default: 1]
  -i, --iterations <ITERATIONS>      Total number of iterations [default: 1]
  -d, --duration <DURATION>          Duration of the test in second
      --headers <HEADERS>            Headers, multi value in format headerName:HeaderValue
      --scenario <SCENARIO>          Scenario file
      --timeout <timeout_ms>         Timeout value in ms, defaults set to 30000
  -h, --help                         Prints help
  -V, --version                      Prints version information
```

#### `--target` `-t`
Specifies the operation and url to make the request, default to GET.<br>
Format: GET https://localhost:3000<br>

#### `--request-body` `-r` Optional
Specifies the path of file with the body to send.<br>

#### `--clients` `-c`
Specifies the number of concurrent calls to be used, defaults to 1.

#### `--iterations` `-i`
Specifies the total number of calls to be performed, default to 1.

#### `--duration` `-d`
Specifies the duration of the test in seconds.

#### `--headers`  Optional
Specifies the headers to be sent.<br>

#### `--scenario`  Optional
Specifies the scenario file in yaml format.<br>

````yaml
target: POST http://localhost:3000/
clients: 50
requests: 1000
headers:
  - key: "bar"
    value: "foo"
  - key: "Content-Type"
    value: "application/json"

body: "{\"firstName\": \"Terry\",
        \"lastName\": \"Medhurst\",
        \"maidenName\": \"Smitham\",
        \"age\": 50}"


````

#### `--help`

Prints help.

#### `--version`

Prints version information.

###### Simple targets

```
goku --target "GET http://localhost:3000"
goku --target http://localhost:3000?foo=bar
goku -c 50 -i 1000 --target http://localhost:3000
goku -c 50 --duration 60 --target http://localhost:3000
```

###### Targets with custom headers

```
goku --target "GET http://localhost:3000" --headers Content-Type:application/json --headers bar:foo 
```

###### Targets with custom bodies

```
goku -c 50 -i 1000 -r body.json --target "POST http://localhost:3000"

```

###### Targets with custom bodies and headers

```
goku -r body.json --target "POST http://localhost:3000" --headers Content-Type:application/json --headers bar:foo 

```

###### Output

```
Concurrency level 50
Time taken 4 seconds
Total requests 1000
Mean request time 169.90099999999998 ms
Max request time 415 ms
Min request time 5 ms
95'th percentile: 319 ms
99.9'th percentile: 367 ms
```

## License

See [LICENSE](LICENSE).

## Donate

If you appreciate all the job done in this project, a small donation is always welcome:

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://www.buymeacoffee.com/jcaro)
