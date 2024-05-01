use rayon::prelude::*;
use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
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
        "CREATE TABLE IF NOT EXISTS documents (
                name TEXT PRIMARY KEY,
                size INTEGER NOT NULL
            )",
        [],
    )?;
    // add document to sql
    conn.execute(
        "INSERT OR REPLACE INTO documents 
            (name, size) VALUES 
            (?1  , ?2)",
        [&table_name, &document.df.iter().count().to_string()],
    )?;

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
    let mut sql_insert = "".to_string();
    for (term, frequency) in &document.df {
        sql_insert.push_str(&format!(
            "INSERT OR REPLACE INTO {} 
                    (term, frequency) VALUES 
                    ('{}'  , {});\n",
            &table_name, &term, &frequency
        ));
    }
    conn.execute(&sql_insert, params![])?;
    return Ok(());
}

fn add_terms_to_sqlite(terms: &TermHash) -> Result<()> {
    // Create db connection
    let conn = Connection::open(DATABASE_PATH)?;
    // Create the table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS terms (
                term TEXT PRIMARY KEY,
                frequency INTEGER NOT NULL
            )",
        [],
    )
    .expect("ERROR: could not create the table for terms");
    // Create db connection
    let mut sql_insert = "".to_string();
    for (term, frequency) in terms {
        sql_insert.push_str(&format!(
            "INSERT OR REPLACE INTO terms 
                    (term, frequency) VALUES 
                    ('{}'  , {});\n",
            &term, &frequency
        ));
    }
    conn.execute(&sql_insert, params![])?;
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

fn query_term(term: &String) -> Result<String> {
    let conn = Connection::open(DATABASE_PATH)?;
    #[derive(Debug, Clone)]
    struct RowData {
        name: String,
        size: usize,
    }
    let mut doc_names = Vec::new();
    let mut doc_sizes = Vec::new();
    conn.prepare(&format!(
        "SELECT name, size FROM documents WHERE name = {}",
        &term
    ))?
    .query_map(&[term], |row| {
        let name = row.get(0)?;
        let size = row.get(1)?;
        Ok(RowData { name, size })
    })?
    .into_iter()
    .for_each(|row| {
        let curr_row = row.expect("ERROR: could not unwrap doc_names");
        doc_names.push(curr_row.name);
        doc_sizes.push(curr_row.size);
    });
    let query: String = doc_names.iter().fold("".to_string(), |name_1, name_2| {
        format!(
            "SELECT frequency FROM {} WHERE term = {}
             UNION
             SELECT frequency FROM {} WHERE term = {}",
            &name_1, &term, &name_2, &term
        )
    });
    dbg!(&query);
    Ok(query)
    // Ok("".to_string())
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
                    // println!("INFO: file: {:?} was added", &path);
                    files.push(path.clone());
                }
            }
        }
    }

    let counter = AtomicUsize::new(0); // Initialize atomic counter
    let unsuccessful_files = Arc::new(Mutex::new(Vec::new()));

    let _ = files
        .par_iter_mut()
        .filter_map(|path| process_file(path))
        .for_each(|doc| {
            let mut total_terms = total_terms.lock().expect("ERROR: Could not lock mutex");
            for (term, frequency) in &doc.df {
                *total_terms.entry(term.to_string()).or_insert(0) += frequency;
            }
            drop(total_terms); // Release the lock
            match create_document_table(&doc) {
                Ok(_) => {
                    let count = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    print!("\rINFO: Successfully indexed document {}", count);
                }
                Err(_) => {
                    let mut unsuccessful_files = unsuccessful_files
                        .lock()
                        .expect("ERROR: Could not lock mutex");
                    unsuccessful_files.push(doc.path);
                }
            };
        });
    println!("");
    if let Err(e) = add_terms_to_sqlite(&total_terms.lock().expect("ERROR: could not lock mutex")) {
        println!("ERROR: {}", e);
    };
    let unsuccessful_files = unsuccessful_files
        .lock()
        .expect("ERROR: could not lock mutex for unsuccessful_files");

    if !unsuccessful_files.is_empty() {
        println!("ERROR: the following files could not be indexed: ",);
        for file in unsuccessful_files.iter() {
            println!(
                "  - {}",
                &file
                    .to_path_buf()
                    .to_str()
                    .expect("ERROR: could not unwrap PathBuf")
            );
        }
    }
    println!("{:?}", query_term(&"of".to_string()));
}

/*
 *
 * TODO: Given a query of terms, make the relavant sql query
 *
 * */
