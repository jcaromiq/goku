# Goku 

Goku is a HTTP load testing tool built out of a need to drill HTTP services with a constant request rate.
It can be used both as a command line utility and a library.

![Goku](https://static1.cbrimages.com/wordpress/wp-content/uploads/2020/01/Goku-Kamehameha-2-1-Cropped-1.jpg?q=50&fit=contain&w=1140&h=&dpr=1.5)

## Install

### Source

You need `rust` installed 
command:

```shell
$ cargo build --release
```

## Versioning

Both the library and the CLI are versioned with [SemVer v2.0.0](https://semver.org/spec/v2.0.0.html).

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md).

## Usage manual

```console
Usage: goku [OPTIONS] --target <TARGET>

Options:
  -t, --target <TARGET>          Url to be request
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
goku --target http://localhost:3000 -c 100 -i 200
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

## License

See [LICENSE](LICENSE).

