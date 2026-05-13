# Goku 
[![Rust](https://github.com/jcaromiq/goku/actions/workflows/ci.yml/badge.svg)](https://github.com/jcaromiq/goku/actions/workflows/ci.yml)
[![](https://img.shields.io/crates/v/goku-bench.svg?ts=2)](https://crates.io/crates/goku-bench)

![Goku](https://raw.githubusercontent.com/jcaromiq/goku/main/assets/goku.png)

Goku is a high-performance, scalable HTTP load-testing tool designed for benchmarking and performance analysis of web services. Inspired by tools like [Drill](https://github.com/fcsonline/drill) and [Vegeta](https://github.com/tsenart/vegeta), Goku offers modern features and simplicity for engineers to simulate and analyze traffic efficiently.

## Features

* Fast and scalable HTTP load testing
* Real-time live stats during long tests
* Rate limiting (`--rps`) for constant-rate load profiles
* Multi-step sequential scenarios (multiple endpoints per test)
* Variable templating in URLs and bodies (`{{uuid}}`, `{{seq}}`, `{{random_int}}`, …)
* Built-in authentication (Bearer token, Basic auth)
* ASCII latency histogram in text output
* `compare` subcommand to diff two benchmark runs
* Output to file (`--output-file`) and per-request log (`--results-log`)
* HTTP/1.1, HTTP/2 support
* Multiple output formats: `text`, `json`, `csv`
* MCP (Model Context Protocol) server for LLM/agent integration

---

## Install CLI

### Automatic download (Linux, OSX, WSL)

```bash
curl -sSL https://raw.githubusercontent.com/jcaromiq/goku/v3.0.0/scripts/install.sh | sh
```

### Using Cargo

```bash
cargo install goku-bench
goku --version
```

### Manual download

Go to the Goku's [GitHub Releases page](https://github.com/jcaromiq/goku/releases) and download the latest `.tar.gz` for your system:

* Linux (x86_64, arm64)
* macOS (x86_64)
* Windows (x86_64)

### From source

```shell
cargo build --release
```

---

## MCP (Model Context Protocol) Support

Goku integrates with the **Model Context Protocol (MCP)** — use Goku programmatically from any LLM agent or MCP-aware client.

### What this enables

- Use Goku from an LLM or AI agent — no manual CLI usage required.
- Combine load testing with automated workflows: trigger a test, gather metrics, and analyze results from within an agent or script.
- Seamless integration into broader toolchains and agentic pipelines.

### Example usage with an LLM

Once Goku is registered as an MCP tool, you can ask your LLM:

> **"Run a load test on https://api.example.com/users with 50 concurrent clients for 30 seconds using HTTP/2, and give me the p95 and p99 latency."**

The LLM will call the `run_benchmark` MCP tool and return the full structured report.

### Install MCP server

```bash
# Automatic download
curl -sSL https://raw.githubusercontent.com/jcaromiq/goku/v3.0.0/scripts/install_mcp.sh | sh

# Or via Cargo
cargo install goku-mcp
```

### MCP tool: `run_benchmark`

The MCP server exposes a single unified tool with the following parameters:

| Parameter | Type | Required | Description |
|---|---|---|---|
| `target` | string | ✅ | URL with optional method prefix. E.g. `"POST http://api.example.com/users"` |
| `clients` | number | ✅ | Number of concurrent workers |
| `requests` | number | — | Total requests (ignored when `duration_secs` is set) |
| `duration_secs` | number | — | Test duration in seconds (alternative to `requests`) |
| `body` | string | — | Request body for POST/PUT/PATCH |
| `headers` | string[] | — | Headers in `"Name:Value"` format |
| `http2` | boolean | — | Use HTTP/2 prior knowledge |
| `ramp_up` | number | — | Seconds to spread worker start |
| `timeout_ms` | number | — | Request timeout in ms (default: 30000) |
| `insecure` | boolean | — | Accept invalid TLS certificates |
| `rps` | number | — | Max requests per second (rate limiting) |

Returns a full JSON report with all latency percentiles, throughput, and status code breakdown.

---

## Versioning

CLI is versioned with [SemVer v2.0.0](https://semver.org/spec/v2.0.0.html).

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md).

---

## Usage manual

```console
Usage: goku [OPTIONS] --target <TARGET>
       goku compare <BASELINE> <CANDIDATE>

Options:
  -v, --verbose                        Runs in verbose mode
  -t, --target <TARGET>                URL to request. Format: [METHOD] <url>  [default: GET]
                                       Example: "POST http://localhost:3000/api"
  -r, --request-body <REQUEST_BODY>    Path to file to use as request body
  -c, --clients <CLIENTS>              Number of concurrent workers [default: 1]
  -i, --iterations <ITERATIONS>        Total number of requests [default: 1]
  -d, --duration <DURATION>            Duration of the test in seconds (alternative to --iterations)
      --headers <HEADERS>              Headers in "Name:Value" format (repeatable)
      --scenario <SCENARIO>            Path to a YAML scenario file
      --timeout <timeout_ms>           Request timeout in ms [default: 30000]
      --http2                          Enable HTTP/2 prior knowledge
      --ramp-up <seconds>              Seconds to spread the start of workers
      --rps <RPS>                      Max requests per second across all clients
      --output <text|json|csv>         Output format [default: text]
      --output-file <PATH>             Write results to file instead of stdout
      --results-log <PATH>             Write per-request CSV log (timestamp, status, latency)
      --live-stats <seconds>           Print live stats every N seconds during the test
      --insecure                       Accept invalid/self-signed TLS certificates
      --auth-bearer <TOKEN>            Set Authorization: Bearer <TOKEN> header
      --auth-basic <USER:PASS>         Set Authorization: Basic <base64> header
      --pool-idle-timeout <seconds>    Connection pool idle timeout [default: 90]
      --disable-keepalive              Disable HTTP keep-alive / connection reuse
  -h, --help                           Print help
  -V, --version                        Print version

Subcommands:
  compare <BASELINE> <CANDIDATE>       Compare two JSON result files and show a diff table
```

---

### Flag reference

#### `--target` `-t`
Specifies the HTTP method and URL. Defaults to `GET` if no method is provided.
```
goku --target "GET http://localhost:3000/"
goku --target "POST http://localhost:3000/api"
goku --target http://localhost:3000          # implicit GET
```

#### `--request-body` `-r` Optional
Path to a file whose contents will be sent as the request body.

#### `--clients` `-c`
Number of concurrent workers. Defaults to `1`.

#### `--iterations` `-i`
Total number of requests to perform across all workers. Defaults to `1`. Cannot be used together with `--duration`.

#### `--duration` `-d`
Run the test for a fixed number of seconds instead of a fixed request count.

#### `--headers` Optional
Add one or more request headers. Repeatable. Format: `Name:Value`.
```
goku --headers Content-Type:application/json --headers X-Api-Key:secret ...
```

#### `--http2` Optional
Force HTTP/2 prior knowledge (skips HTTP/1.1 upgrade negotiation).

#### `--ramp-up` Optional
Spread the start of workers over N seconds to simulate a gradual traffic spike.

#### `--rps` Optional
Limit the total requests per second across all clients. Useful for constant-rate load profiles (similar to Vegeta).
```
goku -c 10 -i 10000 --rps 200 --target http://localhost:3000
```

#### `--output` Optional
Output format. Valid values: `text` (default), `json`, `csv`.

#### `--output-file` Optional
Write results to a file instead of stdout. Works with any `--output` format.
```
goku -c 50 -i 1000 --output json --output-file results.json --target http://localhost:3000
```

#### `--results-log` Optional
Write a per-request CSV log with columns `timestamp_ms,num_client,execution,status,latency_ms`.
```
goku -c 50 -i 1000 --results-log requests.csv --target http://localhost:3000
```

#### `--live-stats` Optional
Print partial metrics (requests, RPS, p50, p95) to stderr every N seconds while the test runs.
```
goku -c 50 --duration 60 --live-stats 5 --target http://localhost:3000
  [live] requests=1250 rps=250.0 p50=45ms p95=120ms
```

#### `--insecure` Optional
Accept invalid or self-signed TLS certificates. **Off by default.**

#### `--auth-bearer` Optional
Inject an `Authorization: Bearer <TOKEN>` header automatically.
```
goku --auth-bearer "eyJhbGci..." --target http://api.example.com
```

#### `--auth-basic` Optional
Inject an `Authorization: Basic <base64>` header from `user:password` credentials.
```
goku --auth-basic "admin:secret" --target http://api.example.com
```

#### `--pool-idle-timeout` Optional
Idle timeout for pooled connections in seconds. Defaults to `90`.

#### `--disable-keepalive` Optional
Disable HTTP keep-alive and connection reuse entirely.

#### `--scenario` Optional
Path to a YAML scenario file. When used, all other flags (except `--output`) are ignored and settings are read from the file.

#### `compare` Subcommand
Compare two benchmark JSON result files and show a colored diff table:
```
goku compare before.json after.json
```

---

### Scenario file format

```yaml
target: POST http://localhost:3000/
clients: 50
requests: 1000
duration: 60          # alternative to requests
http2: true
ramp_up: 5
rps: 500              # optional rate limit
output: json
insecure: false
live_stats: 10        # print live stats every 10s
pool_idle_timeout: 30
disable_keepalive: false

headers:
  - key: "Content-Type"
    value: "application/json"
  - key: "X-Api-Key"
    value: "secret"

body: '{"firstName": "Terry", "lastName": "Medhurst", "age": 50}'

# Optional: built-in authentication
auth:
  type: bearer
  token: "my-token"

# Optional: write results to a file
output_file: results.json
results_log: requests.csv
```

#### Multi-step scenarios

Run multiple endpoints sequentially per worker:

```yaml
clients: 20
duration: 60

steps:
  - target: "GET http://api.example.com/users"
  - target: "POST http://api.example.com/orders"
    body: '{"item": "widget", "qty": 1}'
    headers:
      - key: "Content-Type"
        value: "application/json"
  - target: "GET http://api.example.com/orders/latest"
```

Each worker executes all steps in order, repeating the sequence for the duration of the test.

#### Variable templating

Use dynamic placeholders in URLs and bodies:

| Variable | Description |
|---|---|
| `{{seq}}` | Sequential request number |
| `{{client}}` | Worker ID |
| `{{timestamp}}` | Unix timestamp in ms |
| `{{uuid}}` | Pseudo-random UUID v4 |
| `{{random_int(min,max)}}` | Random integer in `[min, max]` |

```
goku -c 10 -i 100 \
  --target "GET http://api.example.com/users/{{random_int(1,500)}}"

goku -c 10 -i 100 \
  --target "POST http://api.example.com/events" \
  --request-body body.json
# body.json: {"id": "{{uuid}}", "seq": {{seq}}, "ts": {{timestamp}}}
```

---

### Examples

###### Simple targets

```
goku --target "GET http://localhost:3000"
goku --target http://localhost:3000?foo=bar
goku -c 50 -i 1000 --target http://localhost:3000
goku -c 50 --duration 60 --target http://localhost:3000
```

###### With custom headers

```
goku --target "GET http://localhost:3000" \
     --headers Content-Type:application/json \
     --headers X-Api-Key:secret
```

###### With request body

```
goku -c 50 -i 1000 -r body.json --target "POST http://localhost:3000"
```

###### With authentication

```
goku --auth-bearer "my-token" -c 20 -i 500 --target "GET http://api.example.com/protected"
goku --auth-basic "admin:pass" -c 10 -i 100 --target "GET http://api.example.com/admin"
```

###### Rate-limited test

```
goku -c 20 --duration 60 --rps 200 --target http://localhost:3000
```

###### With ramp-up and JSON output to file

```
goku -c 100 -i 5000 --ramp-up 10 --output json --output-file results.json --target http://localhost:3000
```

###### Compare two runs

```
goku -c 50 -i 1000 --output json --output-file before.json --target http://localhost:3000
# deploy new version...
goku -c 50 -i 1000 --output json --output-file after.json --target http://localhost:3000
goku compare before.json after.json
```

###### Output (Text)

```
Concurrency level    50
Time taken           4 seconds
Total requests       1000
Requests/sec         250.00 req/s
Mean                 169.90 ms
Min                  5 ms
Max                  415 ms
p50 (median)         155 ms
p95                  319 ms
p99                  360 ms
p99.9                367 ms

Status codes
  2xx  998
  5xx  2

Latency distribution
      10ms  ████████████████████████████████████████  450
      25ms  █████████████████████████                 280
      50ms  ████████████████                          150
     200ms  ████████                                   80
     415ms  ██                                         40
```

###### Output (JSON)

```json
{
  "concurrency": 50,
  "duration_secs": 4.12,
  "total_requests": 1000,
  "requests_per_sec": 242.72,
  "mean_ms": 169.90,
  "min_ms": 5,
  "max_ms": 415,
  "p50_ms": 155,
  "p95_ms": 319,
  "p99_ms": 360,
  "p999_ms": 367,
  "status_2xx": 998,
  "status_4xx": 0,
  "status_5xx": 2,
  "status_other": 0,
  "network_errors": 0
}
```

---

## License

See [LICENSE](LICENSE).

## Donate

If you appreciate all the job done in this project, a small donation is always welcome:

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://www.buymeacoffee.com/jcaro)
