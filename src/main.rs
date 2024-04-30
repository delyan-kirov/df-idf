use rayon::prelude::*;
use rusqlite::types::Value;
use rusqlite::{params, Connection, Result, ToSql};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

type TermHash = HashMap<String, usize>;

/// The sql table will hold these values
#[derive(Debug)]
struct Document {
    path: PathBuf,
    df: TermHash,
}

fn create_document_table(document: &Document) -> Result<()> {
    let path = Path::new("./data/data.db");
    let conn = Connection::open(path)?;
    // Construct the table name from the document path
    let table_name = document
        .path
        .to_str()
        .expect("Path name should be valid utf8")
        // TODO: escape codes in sqlite text
        .replace("/", "_in_")
        .replace(".", "_dot_"); // Replace '/' and '.' with '_' to make a valid table name

    // Create the table if it doesn't exist
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {} (
                term TEXT PRIMARY KEY,
                frequency INTEGER NOT NULL
            )",
            table_name
        ),
        [],
    )?;
    // Populating the database with values from the HashMap
    for (term, frequency) in &document.df {
        conn.execute(
            &format!(
                "INSERT INTO {} 
                    (term, frequency) VALUES 
                    (?1  , ?2)",
                table_name
            ),
            params![term, frequency],
        )?;
    }
    return Ok(());
}

fn _add_terms_to_sqlite(_doc: &Document) -> Result<()> {
    todo!();
}

fn process_file(path: &Path) -> Option<Document> {
    let mut term_frequencies: TermHash = HashMap::new();
    let file_content = fs::read_to_string(path).ok()?;
    file_content.split_whitespace().for_each(|term| {
        *term_frequencies
            // TODO: lemify
            .entry(term.to_string())
            .or_insert_with(|| 0) += 1;
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
                    println!("INFO: file: {:?} was added", &path);
                    files.push(path.clone());
                }
            }
        }
    }
    let _ = files
        .par_iter()
        .filter_map(|path| process_file(path))
        .for_each(|doc| {
            match create_document_table(&doc) {
                Ok(_) => {
                    println!("INFO: Successfully indexed document {:?}", &doc.path);
                }
                Err(e) => {
                    println!("ERROR: {}", e);
                }
            };
        });
}

/*
 *
 * Extract the content of the files
 * Per file, create a sql table - which is a document type
 * Extract the lemmas of all files in their own sql table of lemma type
 *
 * */
