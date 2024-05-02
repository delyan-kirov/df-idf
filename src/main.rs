use rayon::prelude::*;
use regex::Regex;
use rusqlite::{params, Connection, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

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
    let mut sql_insert = format!(
        "INSERT OR REPLACE INTO {} (term, frequency) VALUES\n",
        table_name,
    )
    .to_string();
    for (index, (term, frequency)) in document.df.iter().enumerate() {
        let comma = if index < document.df.len() - 1 {
            ","
        } else {
            ""
        };
        sql_insert.push_str(&format!(
            " ('{}', {}){}\n",
            term.replace("'", "").replace("\"", ""),
            frequency,
            comma
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

    let mut sql_insert = "INSERT OR REPLACE INTO terms (term, frequency) VALUES\n".to_string();
    for (index, (term, frequency)) in terms.iter().enumerate() {
        let comma = if index < terms.len() - 1 { "," } else { "" };
        let parsed_term = {
            let re = Regex::new(r"[^a-zA-Z]+").unwrap();
            re.replace_all(term, "").to_string()
        };
        sql_insert.push_str(&format!(" ('{}', {}){}\n", parsed_term, frequency, comma));
    }
    // insert data
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

fn query_term(term: &String) -> Result<Vec<(String, f64)>> {
    let conn = Connection::open(DATABASE_PATH)?;
    #[derive(Debug, Clone)]
    struct RowData {
        name: String,
        size: usize,
    }
    let mut docs = HashMap::<String, usize>::new();
    let mut result: Vec<(String, f64)> = Vec::new();
    conn.prepare("SELECT name, size FROM documents")?
        .query_map([], |row| {
            let name = row.get(0)?;
            let size = row.get(1)?;
            Ok(RowData { name, size })
        })?
        .into_iter()
        .for_each(|row| {
            let curr_row = row.expect("ERROR: could not unwrap doc_names");
            docs.insert(curr_row.name.clone(), curr_row.size);
        });
    let query: String = docs
        .keys()
        .map(|name| {
            format!(
                "SELECT frequency, '{}' AS {} FROM {} WHERE term = '{}'\n",
                name, name, name, term
            )
        })
        .collect::<Vec<String>>()
        .join("UNION ALL\n")
        + "\nORDER BY frequency DESC";

    conn.prepare(&query)?
        .query_map([], |row| {
            let frequency: usize = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((frequency, name))
        })?
        .for_each(|data| match data {
            Ok((freq, name)) => {
                if let Some(&total_freq_for_term) = docs.get(&name) {
                    result.push((name.clone(), freq as f64 / total_freq_for_term as f64));
                }
            }
            Err(e) => {
                println!("ERROR: {}", e);
            }
        });
    result.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Ok(result)
}

fn query_many_terms(terms: &Vec<String>) -> Result<Vec<String>> {
    let mut results: Vec<Vec<(String, f64)>> = Vec::new();
    terms.iter().for_each(|term| {
        if let Ok(query) = query_term(term) {
            results.push(query)
        }
    });
    // Find the set intersection of term names in the results
    let mut intersection: Option<HashSet<String>> = None;
    for query in &results {
        let term_set: HashSet<String> = query.iter().map(|(term, _)| term.clone()).collect();
        intersection = match intersection {
            Some(inter) => Some(inter.intersection(&term_set).cloned().collect()),
            None => Some(term_set),
        };
    }
    // Calculate measures for each term in the intersection
    let mut measures: Vec<(String, f64)> = Vec::new();
    match intersection {
        Some(terms) => {
            for term in terms {
                let measure = results.iter().fold(1.0, |acc, result| {
                    let term_freq = result
                        .iter()
                        .find(|(t, _)| *t == term)
                        .map(|(_, freq)| *freq)
                        .unwrap_or(0.0);
                    acc * term_freq
                });
                measures.push((term, measure));
            }
        }
        None => measures = results.concat(),
    }

    // Sort the terms by measure in descending order
    measures.sort_by(|(_, measure1), (_, measure2)| {
        measure2
            .partial_cmp(measure1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Return the sorted terms
    let sorted_terms: Vec<String> = measures.iter().map(|(term, _)| term.clone()).collect();
    Ok(sorted_terms)
}

fn index_files(dir_path: String) {
    let dir_path = Path::new(&dir_path);

    let total_terms: Arc<Mutex<TermHash>> = Arc::new(Mutex::new(HashMap::new()));
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            println!("INFO: file: {:?} was added", &path);
            files.push(path.to_path_buf());
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
}

fn main() {
    index_files("./content".to_string());
    match query_term(&"hello".to_string()) {
        Err(e) => println!("ERROR: {}", e),
        Ok(results) => {
            println!("INFO: number of results: {}", results.iter().count());
            println!("RESULT: ");
            results.iter().enumerate().for_each(|(i, (res, _))| {
                println!(
                    "   {}. {}",
                    i + 1,
                    res.replace("_DOT__IN_content_IN_", "")
                        .replace("_DOT_txt", "")
                );
            });
        }
    }

    query_many_terms(&vec!["advent".to_string(), "news".to_string()])
        .unwrap_or(vec![])
        .iter()
        .for_each(|query| {
            println!(
                "{}",
                query
                    .replace("_DOT__IN_content_IN_", "")
                    .replace("_DOT_txt", "")
            )
        })
}
