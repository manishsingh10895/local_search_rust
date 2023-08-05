use model::{Model, TermFreqPerDoc};
use serde_json;

use std::fs;
use std::io::{BufReader, BufWriter};

use std::process::ExitCode;
use std::{fs::File, path::Path};

use xml::common::{Position, TextPosition};
use xml::reader::{EventReader, XmlEvent};

use crate::lexer::Lexer;
use crate::model::TermFreq;

mod lexer;
mod model;
mod server;
mod snowball;

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

/// Save `TermFreqIndex` to a json file
fn save_model_as_json(model: &Model, index_path: &str) -> Result<(), ()> {
    println!("Saving {index_path}");

    let index_file = File::create(index_path).map_err(|err| {
        eprintln!("ERROR: could not create index file {index_path}: {err}");
    })?;

    serde_json::to_writer(BufWriter::new(index_file), &model).map_err(|err| {
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

    let tf_index: TermFreqPerDoc = serde_json::from_reader(index_file).map_err(|err| {
        eprintln!("ERROR: could not parse index file {index_path}: {err}");
    })?;

    println!(
        "{index_path} contains {count} files",
        count = tf_index.len()
    );

    Ok(())
}

/// Indexes a directory recursively
fn add_folder_to_model(dir_path: &Path, model: &mut Model) -> Result<(), ()> {
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
            add_folder_to_model(&file_path, model)?;
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

        // number of total terms in this file (a term can be repeated)
        let mut num_terms = 0;

        for token in Lexer::new(&content) {
            let term = token;

            if let Some(freq) = tf.get_mut(&term) {
                *freq += 1;
            } else {
                tf.insert(term, 1);
            }

            num_terms += 1;
        }

        // For document frequency of the terms
        // from lexer
        // Increment all the terms by one for every file
        for t in tf.keys() {
            if let Some(freq) = model.df.get_mut(t) {
                *freq += 1;
            } else {
                model.df.insert(t.to_string(), 1);
            }
        }

        model.tfpd.insert(file_path, (num_terms, tf));
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

            let mut model: Model = Default::default();

            add_folder_to_model(Path::new(&dir_path), &mut model)?;
            save_model_as_json(&model, "index.json")?;
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

            let model: Model = serde_json::from_reader(index_file).map_err(|err| {
                eprintln!("ERROR: could not parse index file {index_path}: {err}");
            })?;

            let address = args.next().unwrap_or("127.0.0.1:8000".to_string());

            server::start(&address, &model)?;
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
