use model::Model;
use serde_json;

use std::io::{BufReader, BufWriter};
use std::{fs, thread};

use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::{fs::File, path::Path};

use xml::common::{Position, TextPosition};
use xml::reader::{EventReader, XmlEvent};

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

// parse an md or txt file
fn parse_txt_file(file_path: &Path) -> Result<String, ()> {
    fs::read_to_string(file_path).map_err(|err| {
        eprintln!("ERROR: could not open file {file_path:?}: {err}");
    })
}

// parse s pdf document
fn parse_pdf_file(file_path: &Path) -> Result<String, ()> {
    use poppler::Document;
    use std::io::Read;

    let mut content = Vec::new();

    File::open(file_path)
        .and_then(|mut file| file.read_to_end(&mut content))
        .map_err(|err| {
            eprintln!("ERROR: could not read file {file_path:?}: {err}");
        })?;

    let pdf = Document::from_data(&mut content, None).map_err(|err| {
        eprintln!("ERROR: could not read file {file_path:?}: {err}");
    })?;

    let mut result = String::new();

    let n = pdf.n_pages();

    for i in 0..n {
        let page = pdf.page(i).expect(&format!(
            "{i} is within the bounds of the range of the page"
        ));

        if let Some(content) = page.text() {
            result.push_str(&content);
            result.push(' ');
        }
    }

    Ok(result)
}

/// Check file extension and parses it accordingly
/// Currrently working with `xml`, `html`, `md`, `txt` files only
fn parse_file_by_extension(file_path: &Path) -> Result<String, ()> {
    let extension = file_path
        .extension()
        .ok_or_else(|| {
            eprintln!("ERROR: can't detect file type for {file_path:?}");
        })?
        .to_string_lossy();

    match extension.as_ref() {
        "xhtml" | "xml" | "html" => parse_xml_file(file_path),
        "txt" | "md" => parse_txt_file(file_path),
        "pdf" => parse_pdf_file(file_path),
        _ => {
            eprintln!("ERROR: unsupported file type {file_path:?}");
            Err(())
        }
    }
}

fn usage(program: &str) {
    eprintln!("Usage :{program} [SUBCOMMAND] [OPTIONS]");
    eprintln!("Subcommands:");
    eprintln!("     index <folder> index the <folder> and save the index to index.json");
    eprintln!("     search <index-file> check how many documents are indexed in the file");
    eprintln!(
        "     serve <folder>  [address]             starts local http server with web interfaces"
    );
}

/// Save `TermFreqIndex` to a json file
fn save_model_as_json(model: &Model, index_path: &Path) -> Result<(), ()> {
    println!("Saving {index_path:?}...");

    let index_file = File::create(index_path).map_err(|err| {
        eprintln!("ERROR: could not create index file {index_path:?}: {err}");
    })?;

    serde_json::to_writer(BufWriter::new(index_file), &model).map_err(|err| {
        eprintln!("ERROR: could not serialze index into file {index_path:?}: {err}");
    })?;

    Ok(())
}

/// Reads the created index and prints number of files an index contains
fn check_index(index_path: &str) -> Result<(), ()> {
    println!("Reading {index_path} index file...");

    let index_file = File::open(index_path).map_err(|err| {
        eprintln!("ERROR: could not open index file {index_path}: {err}");
    })?;

    let model: Model = serde_json::from_reader(index_file).map_err(|err| {
        eprintln!("ERROR: could not parse index file {index_path}: {err}");
    })?;

    println!(
        "{index_path} contains {count} files",
        count = model.docs.len()
    );

    Ok(())
}

/// Indexes a directory recursively
fn add_folder_to_model(
    dir_path: &Path,
    model: Arc<Mutex<Model>>,
    processed: &mut usize,
) -> Result<(), ()> {
    let dir = fs::read_dir(dir_path).map_err(|err| {
        eprintln!("ERROR: could not open directory {dir_path:?} for indexing : {err}");
    })?;

    'next_file: for file in dir {
        // 'next_file for naming the loop
        let file = file.map_err(|err| {
            eprintln!("ERROR: could not read next file in directory {dir_path:?}: {err}");
        })?;

        let file_path = file.path();

        // Skip if dot file
        let dot_file = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with("."))
            .unwrap_or(false);

        if dot_file {
            continue 'next_file;
        }

        let file_type = file.file_type().map_err(|err| {
            eprintln!("ERROR: couldnot determine file type for {file_path:?}: {err}");
        })?;

        let last_modified = file
            .metadata()
            .map_err(|err| {
                eprintln!("ERROR: could not get the metadata of the file {file_path:?}: {err}");
            })?
            .modified()
            .map_err(|err| {
                eprintln!(
                    "ERROR: could not get the last modified data for the file {file_path:?}: {err}"
                );
            })?;

        if file_type.is_dir() {
            add_folder_to_model(&file_path, Arc::clone(&model), processed)?;
            continue 'next_file;
        }

        let mut model = model.lock().unwrap();

        if model.requires_reindexing(&file_path, last_modified) {
            println!("Indexing {:?}... ", &file_path);

            let content = match parse_file_by_extension(&file_path) {
                Ok(content) => content.chars().collect::<Vec<_>>(),
                Err(()) => {
                    println!("Err");
                    continue 'next_file;
                }
            };

            model.add_document(file_path, last_modified, &content);

            *processed += 1;
        } else {
            println!(r#"Ignoring {file_path:?} as it is already indexed"#);
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

            let model = Arc::new(Mutex::new(Default::default()));
            let mut processed = 0;

            add_folder_to_model(Path::new(&dir_path), Arc::clone(&model), &mut processed)?;

            let model = model.lock().unwrap();

            save_model_as_json(&model, Path::new("index.json"))?;
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

            let dir_path = args.next().ok_or_else(|| {
                usage(&program);
                eprintln!("ERROR: no directory is provided for {subcommand} subcommand");
            })?;

            let mut index_path = Path::new(&dir_path).to_path_buf();

            index_path.push(".index.json");

            let exists = index_path.try_exists().map_err(|err| {
                eprintln!(
                    "ERROR: could not check for existence for the index {index_path:?}: {err}"
                );
            })?;

            let model: Arc<Mutex<Model>>;
            if exists {
                let index_file = File::open(&index_path).map_err(|err| {
                    eprintln!("ERROR: could not open {index_path:?} {err}");
                })?;

                model = Arc::new(Mutex::new(serde_json::from_reader(index_file).map_err(
                    |err| {
                        eprintln!("ERROR: could not parse index file {index_path:?} {err}");
                    },
                )?));
            } else {
                model = Arc::new(Mutex::new(Default::default()));
            }

            // New scope
            // so that `model` exists in different scope
            {
                let model = Arc::clone(&model);

                thread::spawn(move || {
                    let mut processed = 0;
                    let _ = add_folder_to_model(
                        Path::new(&dir_path),
                        Arc::clone(&model),
                        &mut processed,
                    );

                    if processed > 0 {
                        let model = model.lock().unwrap();
                        save_model_as_json(&model, &index_path).unwrap();
                    }
                });
            }
            // `model` removed from scope

            let address = args.next().unwrap_or("127.0.0.1:8000".to_string());

            server::start(&address, model)?;
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
