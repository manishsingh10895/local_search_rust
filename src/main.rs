use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::{fs::File, path::Path, process::exit};
use xml::reader::{EventReader, XmlEvent};

fn index(_doc_content: &str) -> HashMap<String, usize> {
    unimplemented!();
}

type TermFreq = HashMap<String, usize>;
type TermFreqIndex = HashMap<PathBuf, TermFreq>;

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

    fn next_token(&mut self) -> Option<&'a [char]> {
        // trim whitespaces from left
        self.trim_left();

        if self.content.len() == 0 {
            return None;
        }

        // Lex alphabetic words
        if self.content[0].is_alphabetic() {
            return Some(self.chop_while(|x| x.is_alphabetic()));
        }

        //lex numbers
        if self.content[0].is_numeric() {
            return Some(self.chop_while(|x| x.is_numeric()));
        }

        // Unhandled tokens
        // proceed to next token for next iteration
        //
        Some(self.chop(1))
    }

    fn trim_left(&mut self) {
        while self.content.len() > 0 && self.content[0].is_whitespace() {
            self.content = &self.content[1..];
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = &'a [char];

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

fn read_xml_file<P: AsRef<Path>>(file_path: P) -> std::io::Result<String> {
    let file = File::open(file_path)?;
    let er = EventReader::new(file);

    let mut content = String::new();

    for event in er.into_iter() {
        if let XmlEvent::Characters(text) = event.expect("TODO") {
            content.push_str(&text);
            content.push_str(" ");
        }
    }

    Ok(content)
}

fn main() -> std::io::Result<()> {
    let index_path = "index.json";
    let index_file = File::open(index_path)?;
    println!("Reading {index_path}");

    let global_index: TermFreqIndex = serde_json::from_reader(index_file)?;

    println!("{global_index:?}");

    Ok(())
}

fn main2() -> std::io::Result<()> {
    // let all_doc: HashMap<Path, Box<HashMap<String, usize>>> = HashMap::new();
    let dir_path = "docsgl/gl4";

    let mut global_index = TermFreqIndex::new();

    let dirs = std::fs::read_dir(dir_path).unwrap_or_else(|err| {
        eprintln!("ERROR: cannot read directory: {err}");
        exit(1)
    });

    for file in dirs {
        let file_path = file.unwrap().path();

        let content = read_xml_file(&file_path)
            .unwrap()
            .chars()
            .collect::<Vec<_>>();

        let lexer = Lexer::new(&content);

        let mut tf: TermFreq = TermFreq::new();

        for token in lexer {
            let term = token
                .iter()
                .map(|x| x.to_ascii_uppercase())
                .collect::<String>();

            if let Some(count) = tf.get_mut(&term) {
                *count += 1;
            } else {
                tf.insert(term, 1);
            }
        }

        let mut stats = tf.iter().collect::<Vec<_>>();

        stats.sort_by_key(|(_, f)| *f);

        stats.reverse();

        let _top_tokens: Vec<_> = stats.iter().take(10).collect();

        println!("Indexing {file_path:?} => {size}", size = content.len());

        // for t in top_tokens {
        //     println!("\t{t:?}");
        // }

        global_index.insert(file_path, tf);
    }

    let index_path = "index.json";
    println!("Saving {index_path}");
    let index_file = File::create(index_path).expect("Unable to create index file");

    serde_json::to_writer(index_file, &global_index).expect("Unable to write to {index_path}");

    for (path, tf) in global_index {
        println!("{path:?} has {count} unique tokens", count = tf.len());
    }

    Ok(())
}
