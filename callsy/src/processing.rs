use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;

use reqwest::{Method, Response, Url, Client};
use clap::Parser;

#[derive(Parser)]
pub struct Arguments {
    #[clap(parse(from_os_str), short, default_value = "request.json")]
    request_file : std::path::PathBuf,

    #[clap(parse(from_os_str), short, default_value = "response.json")]
    output_file : std::path::PathBuf,

    #[clap(parse(from_os_str), short)]
    body_output_file : Option<std::path::PathBuf>,
}

pub async fn respond(args : Arguments) -> Result<(), String> {
    
    check_output_file(&args.output_file)?;
    check_body_output_file(&args.body_output_file)?; 
    let input_file = open_input_file(&args.request_file)?;
    let file_contents = read_input_file(input_file)?;
    let raw_request = deserialize_request_data(&file_contents)?;
    let body = get_body(&raw_request)?;
    let body_for_file = body.clone();
    let processed_request = process_request_data(raw_request, body)?;
    let response = make_request(processed_request).await?;
    let output_response = convert_response(response).await?;
    let serialized_response = serialize_response(output_response);
    let output_file = open_output_file(&args.output_file)?;
    write_to_output_file(output_file, serialized_response)?;
    open_and_write_to_body_output_file(&args.body_output_file, body_for_file)?;

    Ok(())
}

#[derive(Deserialize, Debug)]
struct RawRequest {
    url : String,
    method : String,
    headers : HashMap<String, Option<String>>,
    body : Option<String>,
    body_path : Option<std::path::PathBuf>,
}

#[allow(dead_code)]
struct ProcessedRequest {
    url : String,
    method : reqwest::Method,
    headers : HashMap<String, String>,
    body : String,
}

#[derive(Serialize)]
struct OutputResponse {
    headers : HashMap<String, String>,
    status_code : String,
    body : String,
}

fn check_output_file(path : &std::path::PathBuf) -> Result<bool, String> {

    if path.exists() {
        loop {
            print!("Output file {:?} already exists, would you like to overwrite [Y/N]: ", path);

            std::io::stdout().flush().expect("Stdin flush failed.");
            
            let stdin = std::io::stdin();
            let mut buffer = String::with_capacity(2);

            match stdin.read_line(&mut buffer) {
                Ok(_) => {},
                Err(_) => {
                    println!("Failed to read line.");
                    continue;
                },
            }

            match buffer.to_lowercase().trim_end().to_owned().as_str() {
                "y" | "yes" => break Ok(true),
                "n" | "no" => break Err(String::from("Exited due to inability to overwrite existing file.")),
                _ => {},
            }
        }
    }
    else {
        Ok(true)
    }
}

fn check_body_output_file(maybe_path : &Option<std::path::PathBuf>) -> Result<bool, String> {
    if let Some(path) = maybe_path {
        check_output_file(&path)
    }
    else {
        Ok(true)
    }
}

fn open_input_file(path : &std::path::PathBuf) -> Result<std::fs::File, String> {
    match File::open(path) {
        Ok(file) => Ok(file),
        Err(error) => Err(format!("Failed to open input file. OS error: {}", error.raw_os_error().unwrap())),
    }
}

fn read_input_file(mut file : std::fs::File) -> Result<String, String> {
    let mut content = String::new();

    match file.read_to_string(&mut content) {
        Ok(_) => Ok(content),
        Err(error) => Err(format!("Failed to read input file. OS error: {}", error.raw_os_error().unwrap()))
    }
}

fn deserialize_request_data(request_data : &str) -> Result<RawRequest, String> {
    match serde_json::from_str(request_data) {
        Ok(data) => Ok(data),
        Err(error) => Err(format!("Unable to deserialise data from input file at line {}, column {}.", error.line(), error.column())),
    }
}

fn get_body(raw_request : &RawRequest) -> Result<String, String> {
    match (&raw_request.body_path, &raw_request.body) {
        (Some(_), Some(_)) => {
            Err(String::from("Cannot provide both a body and body_path."))
        },
        (Some(path), None) => {
            let mut file = match File::open(path) {
                Ok(file) => file,
                Err(error) => { return Err(format!("Failed to open the body file. OS error: {}", error.raw_os_error().unwrap())); }
            };

            let mut body = String::new();
            match file.read_to_string(&mut body) {
                Ok(_) => Ok(body),
                Err(error) => Err(format!("Failed to read body file. OS error: {}", error.raw_os_error().unwrap()))
            }
        },
        (None, Some(body)) => {
            Ok(body.to_string())
        },
        (None, None) => {
            Ok(String::from(""))
        }
    }    
}

fn process_request_data(raw_request : RawRequest, body : String) -> Result<ProcessedRequest, String> {
    
    fn convert_http_method(raw_request : &RawRequest) -> Result<Method, String> {
        match Method::from_bytes(raw_request.method.to_uppercase().as_bytes()) {
            Ok(method) => Ok(method),
            Err(_) => Err(format!("The provided HTTP method of {} is invalid.", raw_request.method)),
        }
    }

    let method = convert_http_method(&raw_request)?;

    let mut headers = HashMap::new();

    for (header, value) in raw_request.headers {
        match value {
            Some(value) => {
                headers.insert(header, value);
            },
            None => {
                match header.to_lowercase().as_str() {
                    // Auto calculation of null headers where possible.
                    "content-length" => {
                        headers.insert(header, format!("{}", body.len()));
                    },
                    _ => return Err(format!("Cannot autocomplete value of {} header. Try supplying a value directly.", header))
                }
            },
        }
    }

    Ok(ProcessedRequest {
        url : raw_request.url,
        method,
        headers,
        body,
    })
}


async fn make_request(processed_request : ProcessedRequest) -> Result<Response, String> {

    fn parse_url(url : &String) -> Result<reqwest::Url, String> {
        match Url::parse(&url) {
            Ok(url) => Ok(url),
            Err(error) => Err(format!("Error while parsing URL. {}", error)),
        }
    }
    
    let url = parse_url(&processed_request.url)?;
    let body = reqwest::Body::from(processed_request.body); 
    let mut headers = reqwest::header::HeaderMap::new();
    for (k, v) in processed_request.headers.iter() {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            reqwest::header::HeaderValue::from_str(v).unwrap()
        );
    }

    match
        Client::new()
        .request(processed_request.method, url)
        .body(body)
        .headers(headers)
        .send().await {

        Ok(res) => Ok(res),
        Err(error) => Err(format!("Error when sending the request, {}", error)),
    }
}

async fn convert_response(response : Response) -> Result<OutputResponse, String> {
    
    let status_code = String::from(
        response.status().as_str()
    );

    let mut headers = HashMap::new();

    for (k, v) in response.headers().iter() {
        headers.insert(
            String::from(k.as_str()),
            String::from(
                match v.to_str() {
                    Ok(value) => value,
                    Err(_) => "",
                }
            )
        );
    }

    let body = match &response.text().await {
        Ok(body) => body,
        Err(error) => return Err(format!("Failed to get text from response body, {}", error))
    }.clone();

    Ok(OutputResponse {
        headers,
        status_code,
        body,
    })
}

fn serialize_response(output_response : OutputResponse) -> String {
    match serde_json::to_string(&output_response) {
        Ok(result) => result,
        Err(_) => panic!("Internal error, could not serialize JSON data for response"),
    }
}



fn open_output_file(path : &std::path::PathBuf) -> Result<std::fs::File, String> {
    match File::create(path) {
        Ok(file) => Ok(file),
        Err(error) => Err(format!("Failed to create output file. OS error {}", error.raw_os_error().unwrap())),
    }
}

fn write_to_output_file(mut file : std::fs::File, content : String) -> Result<(), String> {
    match file.write(&content.as_bytes()) {
        Ok(_) => Ok(()),
        Err(error) => Err(format!("Failed to write to output file. OS error {}", error))
    }
}

fn open_and_write_to_body_output_file(path : &Option<std::path::PathBuf>, body : String) -> Result<(), String> {
    match path {
        Some(path) => {
            let mut file = match File::create(path) {
                Ok(file) => file,
                Err(error) => {
                    return Err(format!("Failed to create output file. OS error {}", error.raw_os_error().unwrap()));
                }
            };

            match file.write(&body.as_bytes()) {
                Ok(_) => Ok(()),
                Err(error) => Err(format!("Failed to write to output file. OS error {}", error.raw_os_error().unwrap())),
            }
        },
        None => {
            Ok(())
        }
    } 
}
