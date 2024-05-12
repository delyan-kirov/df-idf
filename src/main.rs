use rayon::prelude::*;
use regex::Regex;
use rusqlite::{params, Connection, Result};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::{env, fs, io};
use walkdir::WalkDir;

const DATABASE_PATH: &str = "./data/data.db";

type TermHash = HashMap<String, usize>;

/// The sql table will hold these values
#[derive(Debug)]
struct Document {
    path: PathBuf,
    df: TermHash,
}

fn lemmatize(word: &String) -> String {
    let lemma = stem::get(word);
    match lemma {
        Ok(word_lemma) => return word_lemma,
        Err(e) => {
            println!("ERROR: {}", e);
            return String::new();
        }
    }
}

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
            .entry(lemmatize(&term.to_string()))
            .or_insert_with(|| 0) += 1;
    });
    Some(Document {
        path: path.to_path_buf(),
        df: term_frequencies,
    })
}

/// Compute tf-idf for a single term
fn query_term(term: &String) -> Result<Vec<(String, f64)>> {
    let term = lemmatize(term);
    let conn = Connection::open(DATABASE_PATH)?;
    #[derive(Debug, Clone)]
    struct RowData {
        name: String,
        size: usize,
    }
    let mut docs = HashMap::<String, usize>::new(); // holds all documents and their size
    let mut term_frequency: Vec<(String, f64)> = Vec::new(); // holds the term and updates its
                                                             // frequency

    // extract documents
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

    // extract term
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
                    term_frequency.push((name.clone(), freq as f64 / total_freq_for_term as f64));
                }
            }
            Err(e) => {
                println!("ERROR: {}", e);
            }
        });
    // Computer idf
    let total_term_count = term_frequency.iter().count() as f64;
    let total_doc_count = docs.iter().count() as f64;
    let idf_constant = (total_term_count / total_doc_count).log(std::f64::consts::E);
    // Map idf to the computed df in place
    for (_, freq) in term_frequency.iter_mut() {
        *freq *= idf_constant
    }
    term_frequency.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Ok(term_frequency)
}

/// Computer the tf-idf for a vector of terms
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

/// Generate sqlite tables for the data in the provided directory
fn index_files(dir_path: String) {
    let dir_path = Path::new(&dir_path);

    let total_terms: Arc<Mutex<TermHash>> = Arc::new(Mutex::new(HashMap::new()));
    let mut files: Vec<PathBuf> = Vec::new();

    // walk directory recursively
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

fn find_documents_from_user_query() {
    let mut input = String::new();
    print!("SEARCH: ");
    std::io::stdout()
        .flush()
        .expect("ERROR: could not flush stdout");
    io::stdin()
        .read_line(&mut input)
        .expect("ERROR: failed to read line");

    let search: Vec<String> = input
        .trim()
        .to_string()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let query_result = query_many_terms(&search);
    match query_result {
        Err(e) => eprintln!("ERROR: {e}"),
        Ok(results) => {
            println!("INFO: number of results: {}", results.iter().count());
            println!("RESULT: ");
            results.iter().enumerate().for_each(|(i, res)| {
                println!(
                    "   {}. {}",
                    i + 1,
                    res.replace("_DOT__IN_content_IN_", "")
                        .replace("_DOT_txt", "")
                );
            });
        }
    }
}

fn find_documents_from_commandline(query: &Vec<String>) {
    let query_result = query_many_terms(&query);
    match query_result {
        Err(e) => eprintln!("ERROR: {e}"),
        Ok(results) => {
            println!("INFO: number of results: {}", results.iter().count());
            println!("RESULT: ");
            results.iter().enumerate().for_each(|(i, res)| {
                println!(
                    "   {}. {}",
                    i + 1,
                    res.replace("_DOT__IN_content_IN_", "")
                        .replace("_DOT_txt", "")
                );
            });
        }
    }
}

fn main() {
    let query: Vec<String>;
    let mut args = env::args().into_iter();
    match args.nth(1) {
        Some(a) if a == "-q" => {
            query = args.collect();
            find_documents_from_commandline(&query)
        } // collect query
        Some(a) if a == "-i" => match args.next() {
            Some(dir_path) => {
                index_files(dir_path);
                return ();
            }
            None => {
                println!("ERROR: directory not provided");
                return ();
            }
        }, // index directory
        _ => find_documents_from_user_query(),
    };
}
