use std::path::PathBuf;

use rusqlite::OpenFlags;

fn default_app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.invoicevault.desktop")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut db_path: Option<PathBuf> = None;
    let mut app_data_dir: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--db" => {
                if i + 1 < args.len() {
                    db_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --db requires a path argument");
                    std::process::exit(1);
                }
            }
            "--app-data" => {
                if i + 1 < args.len() {
                    app_data_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --app-data requires a path argument");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                eprintln!("InvoiceVault MCP Server");
                eprintln!();
                eprintln!("Usage: invoicevault-mcp [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --db <path>        Path to SQLite database (default: auto-detect from app data)");
                eprintln!("  --app-data <path>  App data directory (default: ~/.local/share/com.invoicevault.desktop/)");
                eprintln!("  -h, --help         Show this help");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
    }

    let app_data = app_data_dir.unwrap_or_else(default_app_data_dir);
    let db = db_path.unwrap_or_else(|| app_data.join("invoicevault.sqlite3"));

    if !db.exists() {
        eprintln!("Error: Database not found at {}", db.display());
        eprintln!("Specify path with --db <path> or --app-data <path>");
        std::process::exit(1);
    }

    let conn = match rusqlite::Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_WRITE) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to open database: {e}");
            std::process::exit(1);
        }
    };

    // Run storage migrations to ensure schema is up to date
    {
        let mut conn_rw = rusqlite::Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .expect("open db for migrations");
        if let Err(e) = invoicevault_lib::storage::run_migrations(&mut conn_rw) {
            eprintln!("Warning: Failed to run migrations: {e}");
        }
    }

    eprintln!("InvoiceVault MCP Server started (db: {})", db.display());
    invoicevault_lib::mcp::run_server(conn, app_data);
}
