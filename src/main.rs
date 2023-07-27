use serde_json;
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::ops::RangeBounds;
use std::path::PathBuf;
use std::process::ExitCode;
use std::{fs::File, path::Path};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use xml::common::{Position, TextPosition};
use xml::reader::{EventReader, XmlEvent};

type TermFreq = HashMap<String, usize>; // frequency for a token
type TermFreqIndex = HashMap<PathBuf, TermFreq>; // token frequency for a file

// Lexer should contain the parsed document, doesn't modify
#[derive(Debug)]
struct Lexer<'a> {
    content: &'a [char],
}

impl<'a> Lexer<'a> {
    fn new(content: &'a [char]) -> Self {
        Self { content }
    }

    fn chop(&mut self, n: usize) -> &'a [char] {
        let token = &self.content[0..n];
        self.content = &self.content[n..];

        token
    }

    fn chop_while<P>(&mut self, mut predicate: P) -> &'a [char]
    where
        P: FnMut(&char) -> bool,
    {
        let mut n = 0;
        while n < self.content.len() && predicate(&self.content[n]) {
            n += 1;
        }

        return self.chop(n);
    }

    fn next_token(&mut self) -> Option<String> {
        // trim whitespaces from left
        self.trim_left();

        if self.content.len() == 0 {
            return None;
        }

        // Lex alphabetic words
        if self.content[0].is_alphabetic() {
            let term = self
                .chop_while(|x| x.is_alphabetic())
                .iter()
                .map(|x| x.to_ascii_lowercase())
                .collect();
            return Some(term);
        }

        //lex numbers
        if self.content[0].is_numeric() {
            return Some(self.chop_while(|x| x.is_numeric()).iter().collect());
        }

        // Unhandled tokens
        // proceed to next token for next iteration
        //
        Some(self.chop(1).iter().collect())
    }

    fn trim_left(&mut self) {
        while self.content.len() > 0 && self.content[0].is_whitespace() {
            self.content = &self.content[1..];
        }
    }
}

/// Iterator for the Lexer to iterate over
/// generated tokens
impl<'a> Iterator for Lexer<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

// Parse an xml file and returns string containing only relevant characters
fn parse_xml_file(file_path: &Path) -> Result<String, ()> {
    let file = File::open(file_path).map_err(|err| {
        eprintln!("ERROR: could not open file {file_path:?}: {err}",);
    })?;

    let er = EventReader::new(BufReader::new(file));

    let mut content = String::new();

    for event in er.into_iter() {
        let event = event.map_err(|err| {
            let TextPosition { row, column } = err.position();
            let msg = err.msg();
            // prints the location where error was stated
            eprintln!(
                "{file_path}:{row}:{column}: ERROR: {msg}",
                file_path = file_path.display()
            );
        })?;

        if let XmlEvent::Characters(text) = event {
            content.push_str(&text);
            content.push_str(" ");
        }
    }

    Ok(content)
}

fn usage(program: &str) {
    eprintln!("Usage :{program} [SUBCOMMAND] [OPTIONS]");
    eprintln!("Subcommands:");
    eprintln!("     index <folder> index the <folder> and save the index to index.json");
    eprintln!("     search <index-file> check how many documents are indexed in the file");
    eprintln!("     serve <index-file>  [address]             starts local http server with web interfaces");
}

/// Indexes a directory recursively
fn tf_index_of_folder(dir_path: &Path, tf_index: &mut TermFreqIndex) -> Result<(), ()> {
    let dir = fs::read_dir(dir_path).map_err(|err| {
        eprintln!("ERROR: could not open directory {dir_path:?} for indexing : {err}");
    })?;

    'next_file: for file in dir {
        // 'next_file for naming the loop
        let file = file.map_err(|err| {
            eprintln!("ERROR: could not read next file in directory {dir_path:?}: {err}");
        })?;

        let file_path = file.path();

        let file_type = file.file_type().map_err(|err| {
            eprintln!("ERROR: couldnot determine file type for {file_path:?}: {err}");
        })?;

        if file_type.is_dir() {
            tf_index_of_folder(&file_path, tf_index)?;
            continue 'next_file;
        }

        println!("Indexing {:?}... ", &file_path);

        // parse xml file,
        // if error, ignore and carry on to next file
        //
        let content = match parse_xml_file(&file_path) {
            Ok(content) => content.chars().collect::<Vec<_>>(),
            Err(_) => continue 'next_file,
        };

        let mut tf = TermFreq::new();

        for token in Lexer::new(&content) {
            let term = token;

            if let Some(freq) = tf.get_mut(&term) {
                *freq += 1;
            } else {
                tf.insert(term, 1);
            }
        }

        tf_index.insert(file_path, tf);
    }

    Ok(())
}

/// Save `TermFreqIndex` to a json file
fn save_tf_index(tf_index: &TermFreqIndex, index_path: &str) -> Result<(), ()> {
    println!("Saving {index_path}");

    let index_file = File::create(index_path).map_err(|err| {
        eprintln!("ERROR: could not create index file {index_path}: {err}");
    })?;

    serde_json::to_writer(BufWriter::new(index_file), &tf_index).map_err(|err| {
        eprintln!("ERROR: could not serialze index into file {index_path}: {err}");
    })?;

    Ok(())
}

/// Reads the created index and prints number of files an index contains
fn check_index(index_path: &str) -> Result<(), ()> {
    println!("Reading {index_path} index file...");

    let index_file = File::open(index_path).map_err(|err| {
        eprintln!("ERROR: could not open index file {index_path}: {err}");
    })?;

    let tf_index: TermFreqIndex = serde_json::from_reader(index_file).map_err(|err| {
        eprintln!("ERROR: could not parse index file {index_path}: {err}");
    })?;

    println!(
        "{index_path} contains {count} files",
        count = tf_index.len()
    );

    Ok(())
}

fn serve_404(request: Request) -> Result<(), ()> {
    request
        .respond(Response::from_string("404").with_status_code(StatusCode(404)))
        .map_err(|err| {
            eprintln!("Something is not found :{err}");
        })
}

/// Returns the TF for a term in a particular document
fn tf(term: &str, doc: &TermFreq) -> f32 {
    let a = doc.get(term).cloned().unwrap_or(0) as f32;
    let b = doc.iter().map(|(_, f)| *f).sum::<usize>() as f32;

    a / b
}

fn idf(term: &str, docs: &TermFreqIndex) -> f32 {
    let n = docs.len() as f32;
    let m = docs
        .values()
        .filter(|tf| tf.contains_key(term))
        .count()
        .max(1) as f32;

    (n / m).log10() // smaller values are turned negative due to log
}

fn serve_request(tf_index: &TermFreqIndex, mut request: tiny_http::Request) -> Result<(), ()> {
    println!(
        "INFO: Received request method: {:?}, url: {:?}",
        request.method(),
        request.url()
    );

    match (request.method(), request.url()) {
        (Method::Post, "/api/search") => {
            let mut buf = Vec::<u8>::new();
            request.as_reader().read_to_end(&mut buf).map_err(|err| {
                eprintln!("ERROR: Cannot read request body : {err}");
            })?;

            let body = std::str::from_utf8(&buf)
                .map_err(|err| {
                    eprintln!("ERROR: Cannot interpret body at UTF-8 string: {err}");
                })?
                .chars()
                .collect::<Vec<_>>();

            let mut results = Vec::<(&Path, f32)>::new();

            for (path, tf_table) in tf_index {
                let mut rank = 0f32;
                for token in Lexer::new(&body) {
                    rank += tf(&token, &tf_table) * idf(&token, &tf_index);
                    // println!("      {token} => {rank}");
                    // println!("Token {token}, tf => {tf}", tf = tf(&token, tf_table));
                }

                results.push((path, rank));
            }

            results.sort_by(|(_, sum_tf), (_, sum_tf_2)| sum_tf.partial_cmp(sum_tf_2).unwrap());

            results.reverse();

            for (path, rank) in results.iter().take(10) {
                println!("{path:?} => {rank}");
            }

            let json = match serde_json::to_string(&results.iter().take(20).collect::<Vec<_>>()) {
                Ok(json) => json,
                Err(err) => {
                    eprintln!("ERROR: could not convert search results to JSON: {err}");
                    return serve_404(request);
                }
            };

            let content_type_header = Header::from_bytes("Content-Type", "application/json")
                .expect("No garbage in header");

            let _x = request
                .respond(Response::from_string(&json).with_header(content_type_header))
                .unwrap();
        }
        (Method::Get, "/index.js") => {
            let index_js = File::open("index.js").map_err(|err| {
                eprintln!("ERROR: could not find file index.js : {err}");
            })?;

            let content_header = Header::from_bytes("Content-Type", "application/javascript")
                .expect("No garbase in javascript");

            let response = Response::from_file(index_js).with_header(content_header);

            request
                .respond(response)
                .map_err(|err| eprintln!("ERROR: serving javascript: {err}"))?;
        }
        (Method::Get, "/") | (Method::Get, "index.html") => {
            let index_html = File::open("index.html").map_err(|err| {
                eprintln!("ERROR: could not open index.html: {err}");
            })?;

            let content_header =
                Header::from_bytes("Content-Type", "text/html").expect("No garbage in headers");

            let response = Response::from_file(index_html).with_header(content_header);

            request
                .respond(response)
                .unwrap_or_else(|err| eprintln!("Could not server response {err}"));
        }
        _ => {
            serve_404(request)?;
        }
    }

    Ok(())
}

/// Programs's entry point
fn entry() -> Result<(), ()> {
    let mut args = std::env::args();

    let program = args.next().expect("path to program is provided");

    let subcommand = args.next().ok_or_else(|| {
        usage(&program);
        eprintln!("ERROR: no subcommand is provided");
    })?;

    match subcommand.as_str() {
        "index" => {
            let dir_path = args.next().ok_or_else(|| {
                usage(&program);
                eprintln!("ERROR: no directory is provided for {subcommand} subcommand");
            })?;

            let mut tf_index = TermFreqIndex::new();
            tf_index_of_folder(Path::new(&dir_path), &mut tf_index)?;
            save_tf_index(&tf_index, "index.json")?;
        }
        "search" => {
            let index_path = args.next().ok_or_else(|| {
                usage(&program);
                eprintln!("ERROR: no path to index is provided for {subcommand}");
            })?;

            check_index(&index_path)?;
        }
        "serve" => {
            // Start an HTTP server where we can see the indexing
            //

            let index_path = args.next().ok_or_else(|| {
                usage(&program);
                eprintln!("ERROR: no path to index is provided for {subcommand} subcommand");
            })?;

            let index_file = File::open(&index_path).map_err(|err| {
                eprintln!("ERROR: could not open index file {index_path}: {err}");
            })?;

            let tf_index: TermFreqIndex = serde_json::from_reader(index_file).map_err(|err| {
                eprintln!("ERROR: could not parse index file {index_path}: {err}");
            })?;

            let addres = args.next().unwrap_or("127.0.0.1:8000".to_string());
            let server = Server::http(&addres).map_err(|err| {
                eprintln!("ERROR: couldnot start the server at {addres}: {err}");
            })?;

            println!("INFO: Listening at HTTP server at {addres}");

            for request in server.incoming_requests() {
                serve_request(&tf_index, request);
            }
        }
        _ => {
            usage(&program);
            eprintln!("ERROR: unknown subcommand {subcommand}");
            return Err(());
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    match entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
