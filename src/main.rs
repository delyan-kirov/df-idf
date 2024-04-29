use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Document {
    path: PathBuf,
    tf: HashMap<String, f64>,
}

impl Document {
    fn new(path: PathBuf, tf: HashMap<String, f64>) -> Self {
        Document { path, tf }
    }
}

fn process_file(path: &Path) -> Option<Document> {
    let mut term_frequencies: HashMap<String, f64> = HashMap::new();
    let file_content = fs::read_to_string(path).ok()?;

    Some(Document::new(path.to_path_buf(), term_frequencies))
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
