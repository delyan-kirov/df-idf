use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

type TermHash = HashMap<String, usize>;

/// The sql table will hold these values
struct Document {
    path: PathBuf,
    df: TermHash,
}

fn process_file(path: &Path) -> Option<Document> {
    let mut term_frequencies: TermHash = HashMap::new();
    let file_content = fs::read_to_string(path).ok()?;
    file_content.split_whitespace().for_each(|term| {
        term_frequencies.entry(term.to_string()).or_insert(0);
    });
    Some(Document {
        path: path.to_path_buf(),
        df: term_frequencies,
    })
}

fn main() {
    let dir_path = Path::new("./content");
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    files.push(path.clone());
                }
            }
        }
    }
    let documents: Vec<Document> = files
        .par_iter()
        .filter_map(|path| process_file(path))
        // TODO: Add to data sqlite
        .collect();
    println!("{:?}", 1);
}

/*
 *
 * Extract the content of the files
 * Per file, create a sql table - which is a document type
 * Extract the lemmas of all files in their own sql table of lemma type
 *
 * */
