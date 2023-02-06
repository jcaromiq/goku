# Goku 

Goku is a HTTP load testing tool built out of a need to drill HTTP services inspired by [drill](https://github.com/fcsonline/drill) and [vegeta](https://github.com/tsenart/vegeta)  

![Goku](https://static1.cbrimages.com/wordpress/wp-content/uploads/2020/01/Goku-Kamehameha-2-1-Cropped-1.jpg?q=50&fit=contain&w=1140&h=&dpr=1.5)

## Install

### Source

You need `rust` installed 
command:

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
  -t, --target <TARGET>          Url to be requested
  -c, --clients <CLIENTS>        Number of concurrent clients [default: 1]
  -i, --iterations <ITERATIONS>  Total number of iterations [default: 1]
  -h, --help                     Print help
  -V, --version                  Print version

```

#### `--target` `-t`
Specifies the url to make the request

#### `--clients` `-c`
Specifies the number of concurrent calls to be used, defaults to 1.


#### `--iterations` `-i`
Specifies the total number of calls to be performed, default to 1.


#### `--help`
Prints the help and exits.

#### `--version`
Prints the version and exits.

###### Simple targets

```
goku --target http://localhost:3000
goku --target http://localhost:3000?foo=bar
goku --target http://localhost:3000 -c 50 -i 1000
```

###### Targets with custom headers

```
WIP
```

###### Targets with custom bodies

```
WIP
```

###### Targets with custom bodies and headers

```
WIP
```
###### Output
```
Concurrency level 50
Time taken 4 seconds
Total requests 1000
Mean request time 169.90099999999998 ms
Max request time 415 ms
Min request time 5 ms
50'th percentile: 167 ms
90'th percentile: 287 ms
95'th percentile: 319 ms
99.9'th percentile: 367 ms
```

## License

See [LICENSE](LICENSE).

