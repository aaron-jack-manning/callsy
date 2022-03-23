## Callsy

Just a simple HTTP client for the command line that I wrote while learning Rust, which uses JSON files to describe the request and response.

## Building from Source

To build from source and install, simply clone the repository, enter `callsy` folder in terminal and run `cargo build --release` which will create the executable in `target/release` which can then be added to path.

## Sample Usage

Callsy uses a JSON file to describe the request. Such a file is formatted as follows:

`request.json`

```
{
    "url" : "https://somedomain.com",
    "method" : "GET",
    "headers" : {
        "authorization" : "super secret key"
    },
    "body" : "{ \"key\" : \"value\" }"
}
```

We can then run:

```
callsy
```

to create a file `response.json` with the HTTP response data.

Note that while `request.json` is the filename looked for by default, the `-r` option allows this to be specified. Similarly, the `-o` option allows the output file to be specified.

Also note that the `content-length` header can be automatically calculated by specifying it with a value of `null`. This is the only header than can be automatically calculated, other `null` headers will cause an error. Specify the empty string for empty headers.