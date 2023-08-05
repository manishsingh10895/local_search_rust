use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::lexer::Lexer;

pub type TermFreq = HashMap<String, usize>; // frequency for a token
pub type DocFreq = HashMap<String, usize>; // frequency for a token in all the documents
pub type TermFreqPerDoc = HashMap<PathBuf, (usize, TermFreq)>; // token frequency for a file

#[derive(Deserialize, Serialize, Default)]
pub struct Model {
    pub tfpd: TermFreqPerDoc,
    pub df: DocFreq,
}

/// Returns the TF for a term in a particular document
pub fn compute_tf(term: &str, n: usize, doc: &TermFreq) -> f32 {
    let a = doc.get(term).cloned().unwrap_or(0) as f32;
    let b = n as f32;

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

pub fn search_query<'a>(model: &'a Model, query: &'a [char]) -> Vec<(&'a Path, f32)> {
    let mut result = Vec::<(&Path, f32)>::new();

    let tokens = Lexer::new(&query).collect::<Vec<_>>();

    for (path, (n, tf_table)) in &model.tfpd {
        let mut rank = 0f32;

        for token in &tokens {
            rank += compute_tf(&token, *n, &tf_table)
                * compute_idf(&token, model.tfpd.len(), &model.df);
        }

        result.push((path, rank));
    }

    result.sort_by(|(_, sum_tf), (_, sum_tf_2)| sum_tf.partial_cmp(sum_tf_2).unwrap());

    result.reverse();

    result
}
