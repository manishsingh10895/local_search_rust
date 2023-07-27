use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::lexer::Lexer;

pub type TermFreq = HashMap<String, usize>; // frequency for a token
pub type TermFreqIndex = HashMap<PathBuf, TermFreq>; // token frequency for a file

/// Returns the TF for a term in a particular document
pub fn tf(term: &str, doc: &TermFreq) -> f32 {
    let a = doc.get(term).cloned().unwrap_or(0) as f32;
    let b = doc.iter().map(|(_, f)| *f).sum::<usize>() as f32;

    a / b
}

pub fn idf(term: &str, docs: &TermFreqIndex) -> f32 {
    let n = docs.len() as f32;
    let m = docs
        .values()
        .filter(|tf| tf.contains_key(term))
        .count()
        .max(1) as f32;

    (n / m).log10() // smaller values are turned negative due to log
}

pub fn search_query<'a>(tf_index: &'a TermFreqIndex, query: &'a [char]) -> Vec<(&'a Path, f32)> {
    let mut result = Vec::<(&Path, f32)>::new();

    let tokens = Lexer::new(&query).collect::<Vec<_>>();

    for (path, tf_table) in tf_index {
        let mut rank = 0f32;

        for token in &tokens {
            rank += tf(&token, &tf_table) * idf(&token, &tf_index);
        }

        result.push((path, rank));
    }

    result.sort_by(|(_, sum_tf), (_, sum_tf_2)| sum_tf.partial_cmp(sum_tf_2).unwrap());

    result.reverse();

    result
}
