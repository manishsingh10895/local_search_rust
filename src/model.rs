use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

use crate::{lexer::Lexer, snowball};

pub type TermFreq = HashMap<String, usize>; // frequency for a token
pub type DocFreq = HashMap<String, usize>; // frequency for a token in all the documents

#[derive(Deserialize, Serialize, Debug)]
pub struct Doc {
    tf: TermFreq,
    count: usize,
    // SystemTime is platform dependent
    // to an index generated on mac  may not be deserialized
    last_modified: SystemTime,
}

type Docs = HashMap<PathBuf, Doc>; // token frequency for a file

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct Model {
    pub docs: Docs,
    pub df: DocFreq,
}

/// Returns the TF for a term in a particular document
pub fn compute_tf(term: &str, doc: &Doc) -> f32 {
    let b = doc.count as f32;
    let a = doc.tf.get(term).cloned().unwrap_or(0) as f32;

    a / b
}

/// Computes IDF for a term
/// # Arguments
///
/// * `term` term for calculate for
/// * `n_docs` number of total documents in the index
/// * `df` document frequency hash map (map of number of documents a terms appears in)
pub fn compute_idf(term: &str, n_docs: usize, df: &DocFreq) -> f32 {
    let n = n_docs as f32;

    let m = df.get(term).cloned().unwrap_or(1) as f32;

    (n / m).log10() // smaller values are turned negative due to log
}

impl Model {
    /// Remove a file from the model
    /// and also decrements the model's `document frequency` for
    /// all the terms accordingly
    pub fn remove_document(&mut self, file_path: &Path) {
        if let Some(doc) = self.docs.remove(file_path) {
            for t in doc.tf.keys() {
                if let Some(f) = self.df.get_mut(t) {
                    *f -= 1;
                }
            }
        }
    }

    /// A document/file requires reindexing
    /// * If it is already present in the index
    /// * And the file is modified after being indexed
    pub fn requires_reindexing(&mut self, file_path: &Path, last_modified: SystemTime) -> bool {
        if let Some(doc) = self.docs.get(file_path) {
            return doc.last_modified < last_modified;
        }

        return true;
    }

    /// Search for a term `query` in the model
    pub fn search_query(&self, query: &[char]) -> Result<Vec<(PathBuf, f32)>, ()> {
        let mut result = Vec::new();

        let tokens = Lexer::new(&query).collect::<Vec<_>>();

        for (path, doc) in &self.docs {
            let mut rank = 0f32;

            for token in &tokens {
                let mut env = snowball::SnowballEnv::create(&token);
                snowball::algorithms::english_stemmer::stem(&mut env);
                let stemmed = env.get_current().to_string();

                rank +=
                    compute_tf(&stemmed, doc) * compute_idf(&stemmed, self.docs.len(), &self.df);
            }

            if !rank.is_nan() && rank != 0.0 {
                result.push((path.clone(), rank));
            }
        }

        result.sort_by(|(_, sum_tf), (_, sum_tf_2)| sum_tf.partial_cmp(sum_tf_2).unwrap());

        result.reverse();

        Ok(result)
    }

    /// Add a [file]/[document] to the model
    pub fn add_document(
        &mut self,
        file_path: PathBuf,
        last_modified: SystemTime,
        content: &[char],
    ) {
        // if document is already present, removes the model
        self.remove_document(&file_path);

        let mut tf = TermFreq::new();

        let mut count = 0;

        for t in Lexer::new(content) {
            if let Some(f) = tf.get_mut(&t) {
                *f += 1;
            } else {
                tf.insert(t, 1);
            }

            count += 1;
        }

        for t in tf.keys() {
            if let Some(f) = self.df.get_mut(t) {
                *f += 1;
            } else {
                self.df.insert(t.to_string(), 1);
            }
        }

        self.docs.insert(
            file_path,
            Doc {
                tf,
                count,
                last_modified,
            },
        );
    }
}
