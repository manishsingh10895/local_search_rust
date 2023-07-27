use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::lexer::Lexer;

pub type TermFreq = HashMap<String, usize>; // frequency for a token
pub type DocFreq = HashMap<String, usize>; // frequency for a token in all the documents
pub type TermFreqPerDoc = HashMap<PathBuf, TermFreq>; // token frequency for a file

#[derive(Deserialize, Serialize, Default)]
pub struct Model {
    pub tfpd: TermFreqPerDoc,
    pub df: DocFreq,
}

/// Returns the TF for a term in a particular document
pub fn tf(term: &str, doc: &TermFreq) -> f32 {
    let a = doc.get(term).cloned().unwrap_or(0) as f32;
    let b = doc.iter().map(|(_, f)| *f).sum::<usize>() as f32;

    a / b
}

pub fn idf(term: &str, docs: &TermFreqPerDoc) -> f32 {
    let n = docs.len() as f32;
    let m = docs
        .values()
        .filter(|tf| tf.contains_key(term))
        .count()
        .max(1) as f32;

    (n / m).log10() // smaller values are turned negative due to log
}

pub fn search_query<'a>(model: &'a Model, query: &'a [char]) -> Vec<(&'a Path, f32)> {
    let mut result = Vec::<(&Path, f32)>::new();

    let tokens = Lexer::new(&query).collect::<Vec<_>>();

    for (path, tf_table) in &model.tfpd {
        let mut rank = 0f32;

        for token in &tokens {
            rank += tf(&token, &tf_table) * idf(&token, &model.tfpd);
        }

        result.push((path, rank));
    }

    result.sort_by(|(_, sum_tf), (_, sum_tf_2)| sum_tf.partial_cmp(sum_tf_2).unwrap());

    result.reverse();

    result
}
