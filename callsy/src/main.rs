mod processing;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use clap::Parser;

#[tokio::main]
async fn main() {
    let args = crate::processing::Arguments::parse();

    if let Err(message) = crate::processing::respond(args).await {
        println!("Error: {}", message);
    }
}
