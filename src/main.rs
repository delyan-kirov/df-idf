use rayon::prelude::*;
use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type TermHash = HashMap<String, usize>;

/// The sql table will hold these values
#[derive(Debug)]
struct Document {
    path: PathBuf,
    df: TermHash,
}

const DATABASE_PATH: &str = "./data/data.db";

fn create_document_table(document: &Document) -> Result<()> {
    let conn = Connection::open(DATABASE_PATH)?;
    // Construct the table name from the document path
    let table_name = document
        .path
        .to_str()
        .expect("Path name should be valid utf8")
        // Escape codes in sqlite table names
        .replace("/", "_IN_")
        .replace(".", "_DOT_");

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
                "INSERT OR REPLACE INTO {} 
                    (term, frequency) VALUES 
                    (?1  , ?2)",
                table_name
            ),
            params![term, frequency],
        )?;
    }
    return Ok(());
}

fn add_terms_to_sqlite(terms: &TermHash) -> Result<()> {
    // Create db connection
    let conn = Connection::open(DATABASE_PATH)?;
    for (term, frequency) in terms {
        conn.execute(
            "INSERT OR REPLACE INTO terms 
                    (term, frequency) VALUES 
                    (?1  , ?2)",
            params![term, frequency],
        )?;
    }
    Ok(())
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
    let total_terms: Arc<Mutex<TermHash>> = Arc::new(Mutex::new(HashMap::new()));
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
    // Create db connection
    let conn = Connection::open(DATABASE_PATH)
        .expect("ERROR: could not open a connection to the database");
    // Create the table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS terms (
                term TEXT PRIMARY KEY,
                frequency INTEGER NOT NULL
            )",
        [],
    )
    .expect("ERROR: could not create the table for terms");

    let _ = files
        .iter()
        .filter_map(|path| process_file(path))
        .for_each(|doc| {
            let mut total_terms = total_terms.lock().expect("ERROR: Could not lock mutex");
            for (term, frequency) in &doc.df {
                *total_terms.entry(term.to_string()).or_insert(0) += frequency;
            }
            drop(total_terms); // Release the lock
            match create_document_table(&doc) {
                Ok(_) => {
                    println!("INFO: Successfully indexed document {:?}", &doc.path);
                }
                Err(e) => {
                    println!("ERROR: {}", e);
                }
            };
        });
    if let Err(e) = add_terms_to_sqlite(&total_terms.lock().expect("ERROR: could not lock mutex")) {
        println!("ERROR: {}", e);
    };
}

/*
 *
 * TODO: Given a query of terms, make the relavant sql query
 * TODO: Try posgress
 *
 * */
