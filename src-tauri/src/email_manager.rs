use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use chrono::{Duration as ChronoDuration, Utc};
use imap::ClientBuilder;
use mailparse::{parse_mail, ParsedMail};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::{
    event,
    importer::{import_files, ImportJobSummary},
    llm::LlmProviderConfig,
};

struct OAuth2Authenticator {
    user: String,
    access_token: String,
}

impl imap::Authenticator for OAuth2Authenticator {
    type Response = String;
    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("IMAP error: {0}")]
    Imap(String),
    #[error("POP3 error: {0}")]
    Pop3(String),
    #[error("TLS error: {0}")]
    Tls(#[from] native_tls::Error),
    #[error("email source not found: {0}")]
    NotFound(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSource {
    pub id: i64,
    pub name: String,
    pub protocol: String,
    pub imap_host: String,
    pub imap_port: i64,
    pub username: String,
    pub password: String,
    pub auth_method: String,
    pub use_ssl: bool,
    pub folder: String,
    pub name_keywords: String,
    pub max_email_age_days: i64,
    pub enabled: bool,
    pub last_uid: i64,
    pub poll_interval_seconds: i64,
    pub processed_uidls: String,
    pub last_sync_at: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddEmailSourceRequest {
    pub name: Option<String>,
    pub protocol: Option<String>,
    pub imap_host: String,
    pub imap_port: Option<i64>,
    pub username: String,
    pub password: String,
    pub auth_method: Option<String>,
    pub use_ssl: Option<bool>,
    pub folder: Option<String>,
    pub name_keywords: Option<String>,
    pub max_email_age_days: Option<i64>,
    pub poll_interval_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEmailSourceRequest {
    pub name: Option<String>,
    pub protocol: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<i64>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub auth_method: Option<String>,
    pub use_ssl: Option<bool>,
    pub folder: Option<String>,
    pub name_keywords: Option<String>,
    pub max_email_age_days: Option<i64>,
    pub poll_interval_seconds: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailTestResult {
    pub success: bool,
    pub message: String,
    pub folder_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailSyncResult {
    pub source_id: i64,
    pub fetched_count: usize,
    pub imported_count: usize,
    pub jobs: Vec<ImportJobSummary>,
}

// --- POP3 helpers (raw TCP/TLS, no crate dependency) ---

enum Pop3Stream {
    Plain(std::net::TcpStream),
    Tls(native_tls::TlsStream<std::net::TcpStream>),
}

impl std::io::Read for Pop3Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Pop3Stream::Plain(s) => s.read(buf),
            Pop3Stream::Tls(s) => s.read(buf),
        }
    }
}

impl std::io::Write for Pop3Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Pop3Stream::Plain(s) => s.write(buf),
            Pop3Stream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Pop3Stream::Plain(s) => s.flush(),
            Pop3Stream::Tls(s) => s.flush(),
        }
    }
}

fn pop3_connect(host: &str, port: i64, use_ssl: bool) -> Result<Pop3Stream, EmailError> {
    let tcp = std::net::TcpStream::connect(format!("{host}:{port}"))
        .map_err(|e| EmailError::Pop3(format!("tcp connect: {e}")))?;
    if use_ssl {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| EmailError::Pop3(format!("tls build: {e}")))?;
        let stream = tls
            .connect(host, tcp)
            .map_err(|e| EmailError::Pop3(format!("tls connect: {e}")))?;
        Ok(Pop3Stream::Tls(stream))
    } else {
        Ok(Pop3Stream::Plain(tcp))
    }
}

fn pop3_read_line(conn: &mut Pop3Stream) -> Result<String, EmailError> {
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(conn);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| EmailError::Pop3(format!("read: {e}")))?;
    Ok(line.trim_end().to_string())
}

fn pop3_send_cmd(conn: &mut Pop3Stream, cmd: &str) -> Result<String, EmailError> {
    use std::io::Write;
    conn.write_all(cmd.as_bytes())
        .map_err(|e| EmailError::Pop3(format!("write: {e}")))?;
    conn.flush()
        .map_err(|e| EmailError::Pop3(format!("flush: {e}")))?;
    let resp = pop3_read_line(conn)?;
    if resp.starts_with("+OK") {
        Ok(resp)
    } else if resp.to_lowercase().contains("err") && (resp.to_lowercase().contains("auth") || resp.to_lowercase().contains("pass") || resp.to_lowercase().contains("user")) {
        Err(EmailError::Pop3(format!(
            "认证失败: {resp}。如果使用国内邮箱（163/QQ/Yeah等），请使用「授权码」而非登录密码"
        )))
    } else {
        Err(EmailError::Pop3(format!("command failed: {resp}")))
    }
}

fn wrap_imap_login_error(e: imap::Error) -> EmailError {
    let msg = e.to_string();
    if msg.contains("LOGIN") || msg.contains("login") || msg.contains("AUTHENTICATE") || msg.contains("authenticate") {
        EmailError::Imap(format!(
            "{msg}。如果使用国内邮箱（163/QQ/Yeah等），请使用「授权码」而非登录密码"
        ))
    } else {
        EmailError::Imap(format!("login: {msg}"))
    }
}

/// Send ID command after login. Coremail servers (163.com etc.) require this
/// before SELECT/EXAMINE will be allowed.
fn imap_send_id(session: &mut imap::Session<imap::Connection>) {
    let _ = session.run_command_and_read_response(
        "ID (\"name\" \"InvoiceVault\" \"version\" \"1.0\")"
    );
}

fn pop3_multiline_cmd(conn: &mut Pop3Stream, cmd: &str) -> Result<Vec<String>, EmailError> {
    use std::io::{BufRead, Write};
    conn.write_all(cmd.as_bytes())
        .map_err(|e| EmailError::Pop3(format!("write: {e}")))?;
    conn.flush()
        .map_err(|e| EmailError::Pop3(format!("flush: {e}")))?;

    let mut reader = std::io::BufReader::new(conn);
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(|e| EmailError::Pop3(format!("read: {e}")))?;
    if !first_line.starts_with("+OK") {
        return Err(EmailError::Pop3(format!(
            "command failed: {}",
            first_line.trim()
        )));
    }

    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| EmailError::Pop3(format!("read: {e}")))?;
        let trimmed = line.trim_end();
        if trimmed == "." {
            break;
        }
        lines.push(trimmed.to_string());
    }
    Ok(lines)
}

// ---

pub struct EmailManager {
    db: Arc<Mutex<Connection>>,
    raw_dir: PathBuf,
    thumbnails_dir: PathBuf,
    llm_config: Arc<Mutex<Option<LlmProviderConfig>>>,
    llm_audit_enabled: Arc<Mutex<bool>>,
}

impl EmailManager {
    pub fn new(
        db: Arc<Mutex<Connection>>,
        raw_dir: PathBuf,
        thumbnails_dir: PathBuf,
        llm_config: Arc<Mutex<Option<LlmProviderConfig>>>,
        llm_audit_enabled: Arc<Mutex<bool>>,
    ) -> Self {
        Self {
            db,
            raw_dir,
            thumbnails_dir,
            llm_config,
            llm_audit_enabled,
        }
    }

    // --- CRUD ---

    pub fn add_email_source(
        &self,
        request: AddEmailSourceRequest,
    ) -> Result<EmailSource, EmailError> {
        let name = request.name.unwrap_or_default();
        let protocol = request.protocol.unwrap_or_else(|| "imap".into());
        let imap_port = request
            .imap_port
            .unwrap_or(if protocol == "pop3" { 995 } else { 993 });
        let auth_method = request.auth_method.unwrap_or_else(|| "password".into());
        let use_ssl = request.use_ssl.unwrap_or(true);
        let folder = request.folder.unwrap_or_else(|| "INBOX".into());
        let name_keywords = request.name_keywords.unwrap_or_default();
        let max_email_age_days = request.max_email_age_days.unwrap_or(30);
        let poll_interval = request.poll_interval_seconds.unwrap_or(300);

        let db = self.db.lock().expect("db lock");
        db.execute(
            "INSERT INTO email_sources (name, protocol, imap_host, imap_port, username, password, auth_method, use_ssl, folder, name_keywords, max_email_age_days, poll_interval_seconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![name, protocol, request.imap_host, imap_port, request.username, request.password,
                   auth_method, use_ssl as i32, folder, name_keywords, max_email_age_days, poll_interval],
        )?;
        let id = db.last_insert_rowid();
        drop(db);

        self.get_email_source(id)
    }

    pub fn update_email_source(
        &self,
        id: i64,
        request: UpdateEmailSourceRequest,
    ) -> Result<EmailSource, EmailError> {
        let db = self.db.lock().expect("db lock");
        let mut sets: Vec<String> = Vec::new();
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        macro_rules! set_field {
            ($field:expr, $col:expr) => {
                if let Some(v) = $field {
                    vals.push(Box::new(v));
                    sets.push(format!("{} = ?{}", $col, vals.len()));
                }
            };
        }
        macro_rules! set_str {
            ($field:expr, $col:expr) => {
                if let Some(ref v) = $field {
                    vals.push(Box::new(v.clone()));
                    sets.push(format!("{} = ?{}", $col, vals.len()));
                }
            };
        }

        set_str!(request.name, "name");
        set_str!(request.protocol, "protocol");
        set_str!(request.imap_host, "imap_host");
        set_field!(request.imap_port, "imap_port");
        set_str!(request.username, "username");
        set_str!(request.password, "password");
        set_str!(request.auth_method, "auth_method");
        if let Some(v) = request.use_ssl {
            vals.push(Box::new(v as i32));
            sets.push(format!("use_ssl = ?{}", vals.len()));
        }
        set_str!(request.folder, "folder");
        set_str!(request.name_keywords, "name_keywords");
        set_field!(request.max_email_age_days, "max_email_age_days");
        set_field!(request.poll_interval_seconds, "poll_interval_seconds");
        if let Some(v) = request.enabled {
            vals.push(Box::new(v as i32));
            sets.push(format!("enabled = ?{}", vals.len()));
        }

        if sets.is_empty() {
            return self.get_email_source(id);
        }

        let sql = format!(
            "UPDATE email_sources SET {}, updated_at = CURRENT_TIMESTAMP WHERE id = ?{}",
            sets.join(", "),
            vals.len() + 1,
        );
        vals.push(Box::new(id));
        let refs: Vec<&dyn rusqlite::types::ToSql> = vals.iter().map(|v| v.as_ref()).collect();
        db.execute(&sql, refs.as_slice())?;
        drop(db);

        self.get_email_source(id)
    }

    pub fn remove_email_source(&self, id: i64) -> Result<(), EmailError> {
        let db = self.db.lock().expect("db lock");
        let affected = db.execute("DELETE FROM email_sources WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(EmailError::NotFound(id));
        }
        Ok(())
    }

    pub fn list_email_sources(&self) -> Result<Vec<EmailSource>, EmailError> {
        let db = self.db.lock().expect("db lock");
        let mut stmt = db.prepare(
            "SELECT id, name, protocol, imap_host, imap_port, username, password, auth_method, use_ssl, folder,
                    name_keywords, max_email_age_days, enabled, last_uid, poll_interval_seconds,
                    processed_uidls, last_sync_at, status, error_message, created_at, updated_at
             FROM email_sources ORDER BY id",
        )?;
        let sources: Vec<EmailSource> = stmt
            .query_map([], |row| {
                Ok(EmailSource {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol: row.get(2)?,
                    imap_host: row.get(3)?,
                    imap_port: row.get(4)?,
                    username: row.get(5)?,
                    password: row.get(6)?,
                    auth_method: row.get(7)?,
                    use_ssl: row.get::<_, i32>(8)? != 0,
                    folder: row.get(9)?,
                    name_keywords: row.get(10)?,
                    max_email_age_days: row.get(11)?,
                    enabled: row.get::<_, i32>(12)? != 0,
                    last_uid: row.get(13)?,
                    poll_interval_seconds: row.get(14)?,
                    processed_uidls: row.get(15)?,
                    last_sync_at: row.get(16)?,
                    status: row.get(17)?,
                    error_message: row.get(18)?,
                    created_at: row.get(19)?,
                    updated_at: row.get(20)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sources)
    }

    pub fn toggle_email_source(&self, id: i64, enabled: bool) -> Result<EmailSource, EmailError> {
        let db = self.db.lock().expect("db lock");
        db.execute(
            "UPDATE email_sources SET enabled = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id, enabled as i32],
        )?;
        drop(db);
        self.get_email_source(id)
    }

    fn get_email_source(&self, id: i64) -> Result<EmailSource, EmailError> {
        let db = self.db.lock().expect("db lock");
        let source = db.query_row(
            "SELECT id, name, protocol, imap_host, imap_port, username, password, auth_method, use_ssl, folder,
                    name_keywords, max_email_age_days, enabled, last_uid, poll_interval_seconds,
                    processed_uidls, last_sync_at, status, error_message, created_at, updated_at
             FROM email_sources WHERE id = ?1",
            [id],
            |row| {
                Ok(EmailSource {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol: row.get(2)?,
                    imap_host: row.get(3)?,
                    imap_port: row.get(4)?,
                    username: row.get(5)?,
                    password: row.get(6)?,
                    auth_method: row.get(7)?,
                    use_ssl: row.get::<_, i32>(8)? != 0,
                    folder: row.get(9)?,
                    name_keywords: row.get(10)?,
                    max_email_age_days: row.get(11)?,
                    enabled: row.get::<_, i32>(12)? != 0,
                    last_uid: row.get(13)?,
                    poll_interval_seconds: row.get(14)?,
                    processed_uidls: row.get(15)?,
                    last_sync_at: row.get(16)?,
                    status: row.get(17)?,
                    error_message: row.get(18)?,
                    created_at: row.get(19)?,
                    updated_at: row.get(20)?,
                })
            },
        )?;
        Ok(source)
    }

    // --- Test Connection ---

    pub fn test_connection(
        &self,
        protocol: &str,
        host: &str,
        port: i64,
        username: &str,
        password: &str,
        auth_method: &str,
        use_ssl: bool,
        folder: &str,
    ) -> Result<EmailTestResult, EmailError> {
        if protocol == "pop3" {
            let mut conn = pop3_connect(host, port, use_ssl)?;
            pop3_read_line(&mut conn)?; // greeting
            pop3_send_cmd(&mut conn, &format!("USER {username}\r\n"))?;
            pop3_send_cmd(&mut conn, &format!("PASS {password}\r\n"))?;
            let lines = pop3_multiline_cmd(&mut conn, "UIDL\r\n")?;
            let count = lines.len() as i64;
            pop3_send_cmd(&mut conn, "QUIT\r\n").ok();

            return Ok(EmailTestResult {
                success: true,
                message: format!("POP3 连接成功，收件箱中有 {count} 封邮件"),
                folder_count: Some(count),
            });
        }

        let client = ClientBuilder::new(host, port as u16)
            .connect()
            .map_err(|e| EmailError::Imap(format!("connect: {e}")))?;

        let mut session = if auth_method == "oauth2" {
            let auth = OAuth2Authenticator {
                user: username.to_string(),
                access_token: password.to_string(),
            };
            client
                .authenticate("XOAUTH2", &auth)
                .map_err(|(e, _)| EmailError::Imap(format!("OAuth2 认证失败: {e}")))?
        } else {
            client
                .login(username, password)
                .map_err(|(e, _)| wrap_imap_login_error(e))?
        };

        imap_send_id(&mut session);

        let mailbox = session
            .select(folder)
            .map_err(|e| EmailError::Imap(format!("select folder: {e}")))?;

        let count = mailbox.exists as i64;

        let _ = session.logout();

        Ok(EmailTestResult {
            success: true,
            message: format!("连接成功，邮箱 {} 中有 {} 封邮件", folder, count),
            folder_count: Some(count),
        })
    }

    // --- Sync ---

    pub fn sync_email_source(&self, id: i64) -> Result<EmailSyncResult, EmailError> {
        let source = self.get_email_source(id)?;

        if !source.enabled {
            return Ok(EmailSyncResult {
                source_id: id,
                fetched_count: 0,
                imported_count: 0,
                jobs: vec![],
            });
        }

        // Mark as syncing
        {
            let db = self.db.lock().expect("db lock");
            db.execute(
                "UPDATE email_sources SET status = 'syncing', error_message = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                [id],
            )?;
        }

        let result = if source.protocol == "pop3" {
            self.do_sync_pop3(&source)
        } else {
            self.do_sync(&source)
        };

        match &result {
            Ok(sync_result) => {
                let db = self.db.lock().expect("db lock");
                let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                db.execute(
                    "UPDATE email_sources SET status = 'idle', last_sync_at = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                    params![id, now],
                )?;
                info!(
                    "Email sync done for source {}: fetched={}, imported={}",
                    id, sync_result.fetched_count, sync_result.imported_count
                );
            }
            Err(e) => {
                let db = self.db.lock().expect("db lock");
                db.execute(
                    "UPDATE email_sources SET status = 'error', error_message = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                    params![id, e.to_string()],
                )?;
                error!("Email sync failed for source {}: {e}", id);
            }
        }

        result
    }

    pub fn sync_all_enabled(&self) -> Result<Vec<EmailSyncResult>, EmailError> {
        let sources = self.list_email_sources()?;
        let mut results = Vec::new();
        for source in sources {
            if source.enabled {
                match self.sync_email_source(source.id) {
                    Ok(r) => results.push(r),
                    Err(e) => {
                        warn!("Email sync failed for source {}: {e}", source.id);
                    }
                }
            }
        }
        Ok(results)
    }

    fn do_sync(&self, source: &EmailSource) -> Result<EmailSyncResult, EmailError> {
        let client = ClientBuilder::new(&source.imap_host, source.imap_port as u16)
            .connect()
            .map_err(|e| EmailError::Imap(format!("connect: {e}")))?;

        let mut session = if source.auth_method == "oauth2" {
            let auth = OAuth2Authenticator {
                user: source.username.clone(),
                access_token: source.password.clone(),
            };
            client
                .authenticate("XOAUTH2", &auth)
                .map_err(|(e, _)| EmailError::Imap(format!("OAuth2 认证失败: {e}")))?
        } else {
            client
                .login(&source.username, &source.password)
                .map_err(|(e, _)| wrap_imap_login_error(e))?
        };

        imap_send_id(&mut session);

        session
            .select(&source.folder)
            .map_err(|e| EmailError::Imap(format!("select: {e}")))?;

        // Build UID search criteria
        let mut criteria = String::new();
        if source.last_uid > 0 {
            criteria.push_str(&format!("UID {}:*", source.last_uid + 1));
        }
        if source.max_email_age_days > 0 {
            let since = Utc::now() - ChronoDuration::days(source.max_email_age_days);
            let since_str = since.format("%d-%b-%Y").to_string();
            if criteria.is_empty() {
                criteria.push_str(&format!("SINCE {}", since_str));
            } else {
                criteria.push_str(&format!(" SINCE {}", since_str));
            }
        }

        let uids: Vec<u32> = if criteria.is_empty() {
            session
                .uid_search("1:*")
                .map_err(|e| EmailError::Imap(format!("search: {e}")))?
                .into_iter()
                .collect()
        } else {
            session
                .uid_search(&criteria)
                .map_err(|e| EmailError::Imap(format!("search: {e}")))?
                .into_iter()
                .collect()
        };

        let fetched_count = uids.len();
        if uids.is_empty() {
            let _ = session.logout();
            return Ok(EmailSyncResult {
                source_id: source.id,
                fetched_count: 0,
                imported_count: 0,
                jobs: vec![],
            });
        }

        // Parse keywords for attachment filtering
        let keywords: Vec<String> = source
            .name_keywords
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        // Fetch emails in batches
        let uid_list: Vec<String> = uids.iter().map(|u: &u32| u.to_string()).collect();
        let uid_set = uid_list.join(",");

        let messages = session
            .uid_fetch(&uid_set, "(UID RFC822)")
            .map_err(|e| EmailError::Imap(format!("fetch: {e}")))?;

        let mut max_uid = source.last_uid;
        let mut all_jobs: Vec<ImportJobSummary> = Vec::new();
        let mut tmp_files: Vec<PathBuf> = Vec::new();

        for msg in messages.iter() {
            let uid = msg.uid.unwrap_or(0);
            let uid_i64 = uid as i64;
            if uid_i64 > max_uid {
                max_uid = uid_i64;
            }

            let body = match msg.body() {
                Some(b) => b,
                None => continue,
            };

            let parsed = match parse_mail(body) {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to parse email UID {uid}: {e}");
                    continue;
                }
            };

            self.extract_attachments(&parsed, &keywords, &mut tmp_files);
        }

        let _ = session.logout();

        // Import all extracted attachments
        let imported_count = if !tmp_files.is_empty() {
            let path_strs: Vec<String> = tmp_files
                .iter()
                .filter_map(|p| p.to_str().map(String::from))
                .collect();

            let (jobs, raw_file_ids) = match self.db.lock() {
                Ok(mut conn) => {
                    let jobs = import_files(&mut conn, &self.raw_dir, path_strs, "email")
                        .unwrap_or_default();
                    let ids: Vec<i64> = jobs
                        .iter()
                        .filter(|j| j.status == "imported" && j.raw_file_id.is_some())
                        .filter_map(|j| j.raw_file_id)
                        .collect();

                    let total = jobs.len();
                    let success = jobs.iter().filter(|j| j.status == "imported").count();
                    let dups = jobs.iter().filter(|j| j.status == "duplicate").count();
                    let failed = jobs.iter().filter(|j| j.status == "failed").count();
                    let _ =
                        event::record_import_event(&conn, total, success, dups, failed, &[], &ids);

                    (jobs, ids)
                }
                Err(_) => (vec![], vec![]),
            };

            // Auto-recognize imported files
            if !raw_file_ids.is_empty() {
                if let Some(config) = self.llm_config.lock().ok().and_then(|c| c.clone()) {
                    let db = Arc::clone(&self.db);
                    let thumbnails_dir = self.thumbnails_dir.clone();
                    let audit = (*self.llm_audit_enabled.lock().expect("lock")).then(|| {
                        crate::llm::LlmAuditConfig {
                            dir: std::path::PathBuf::from("audit"), // placeholder
                        }
                    });

                    tauri::async_runtime::spawn(async move {
                        for raw_file_id in raw_file_ids {
                            crate::watcher::recognize_raw_file_async(
                                &db,
                                &thumbnails_dir,
                                raw_file_id,
                                &config,
                                audit.as_ref(),
                            )
                            .await;
                        }
                    });
                }
            }

            let imported = jobs.iter().filter(|j| j.status == "imported").count();
            all_jobs = jobs;

            // Clean up temp files
            for tmp in &tmp_files {
                let _ = std::fs::remove_file(tmp);
            }

            imported
        } else {
            0
        };

        // Update last_uid
        {
            let db = self.db.lock().expect("db lock");
            db.execute(
                "UPDATE email_sources SET last_uid = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                params![source.id, max_uid as i64],
            )?;
        }

        Ok(EmailSyncResult {
            source_id: source.id,
            fetched_count,
            imported_count,
            jobs: all_jobs,
        })
    }

    fn do_sync_pop3(&self, source: &EmailSource) -> Result<EmailSyncResult, EmailError> {
        let mut conn = pop3_connect(&source.imap_host, source.imap_port, source.use_ssl)?;
        let _greeting = pop3_read_line(&mut conn)?;
        pop3_send_cmd(&mut conn, &format!("USER {}\r\n", source.username))?;
        pop3_send_cmd(&mut conn, &format!("PASS {}\r\n", source.password))?;

        // Get UIDL list for all messages
        let uidl_lines = pop3_multiline_cmd(&mut conn, "UIDL\r\n")?;

        // Parse UIDL response into (msg_num, uidl) pairs
        let all_uidls: Vec<(u32, String)> = uidl_lines
            .iter()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let num = parts[0].parse::<u32>().ok()?;
                    Some((num, parts[1].trim().to_string()))
                } else {
                    None
                }
            })
            .collect();

        // Compare with locally processed UIDLs
        let processed: std::collections::HashSet<String> = source
            .processed_uidls
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let new_msgs: Vec<(u32, String)> = all_uidls
            .iter()
            .filter(|(_, uidl)| !processed.contains(uidl))
            .cloned()
            .collect();

        let fetched_count = new_msgs.len();
        if new_msgs.is_empty() {
            pop3_send_cmd(&mut conn, "QUIT\r\n").ok();
            return Ok(EmailSyncResult {
                source_id: source.id,
                fetched_count: 0,
                imported_count: 0,
                jobs: vec![],
            });
        }

        let keywords: Vec<String> = source
            .name_keywords
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut all_jobs: Vec<ImportJobSummary> = Vec::new();
        let mut tmp_files: Vec<PathBuf> = Vec::new();
        let mut new_processed = processed;

        for (msg_num, uidl) in &new_msgs {
            // Check age filter
            if source.max_email_age_days > 0 {
                // POP3 doesn't have server-side date filter, try to get headers with TOP
                if let Ok(top_lines) =
                    pop3_multiline_cmd(&mut conn, &format!("TOP {} 30\r\n", msg_num))
                {
                    let header_text = top_lines.join("\n");
                    if let Some(date_str) = header_text
                        .lines()
                        .find(|l| l.to_lowercase().starts_with("date:"))
                        .map(|l| l[5..].trim())
                    {
                        if let Ok(parsed_date) = mailparse::dateparse(date_str) {
                            let email_date = chrono::DateTime::from_timestamp(parsed_date, 0)
                                .map(|d| d.naive_utc());
                            if let Some(email_date) = email_date {
                                let cutoff = Utc::now().naive_utc()
                                    - ChronoDuration::days(source.max_email_age_days);
                                if email_date < cutoff {
                                    new_processed.insert(uidl.clone());
                                    continue;
                                }
                            }
                        }
                    }
                }
            }

            // Retrieve the full email
            let raw_lines = match pop3_multiline_cmd(&mut conn, &format!("RETR {}\r\n", msg_num)) {
                Ok(lines) => lines,
                Err(e) => {
                    warn!("POP3: failed to retrieve message {msg_num}: {e}");
                    continue;
                }
            };
            let raw = raw_lines.join("\r\n");

            let parsed = match mailparse::parse_mail(raw.as_bytes()) {
                Ok(p) => p,
                Err(e) => {
                    warn!("POP3: failed to parse message {msg_num}: {e}");
                    continue;
                }
            };

            self.extract_attachments(&parsed, &keywords, &mut tmp_files);
            new_processed.insert(uidl.clone());
        }

        pop3_send_cmd(&mut conn, "QUIT\r\n").ok();

        // Import extracted attachments
        let imported_count = if !tmp_files.is_empty() {
            let path_strs: Vec<String> = tmp_files
                .iter()
                .filter_map(|p| p.to_str().map(String::from))
                .collect();

            let (jobs, raw_file_ids) = match self.db.lock() {
                Ok(mut conn) => {
                    let jobs = import_files(&mut conn, &self.raw_dir, path_strs, "email")
                        .unwrap_or_default();
                    let ids: Vec<i64> = jobs
                        .iter()
                        .filter(|j| j.status == "imported" && j.raw_file_id.is_some())
                        .filter_map(|j| j.raw_file_id)
                        .collect();

                    let total = jobs.len();
                    let success = jobs.iter().filter(|j| j.status == "imported").count();
                    let dups = jobs.iter().filter(|j| j.status == "duplicate").count();
                    let failed = jobs.iter().filter(|j| j.status == "failed").count();
                    let _ =
                        event::record_import_event(&conn, total, success, dups, failed, &[], &ids);

                    (jobs, ids)
                }
                Err(_) => (vec![], vec![]),
            };

            // Auto-recognize
            if !raw_file_ids.is_empty() {
                if let Some(config) = self.llm_config.lock().ok().and_then(|c| c.clone()) {
                    let db = Arc::clone(&self.db);
                    let thumbnails_dir = self.thumbnails_dir.clone();
                    let audit = (*self.llm_audit_enabled.lock().expect("lock")).then(|| {
                        crate::llm::LlmAuditConfig {
                            dir: std::path::PathBuf::from("audit"),
                        }
                    });

                    tauri::async_runtime::spawn(async move {
                        for raw_file_id in raw_file_ids {
                            crate::watcher::recognize_raw_file_async(
                                &db,
                                &thumbnails_dir,
                                raw_file_id,
                                &config,
                                audit.as_ref(),
                            )
                            .await;
                        }
                    });
                }
            }

            let imported = jobs.iter().filter(|j| j.status == "imported").count();
            all_jobs = jobs;

            for tmp in &tmp_files {
                let _ = std::fs::remove_file(tmp);
            }

            imported
        } else {
            0
        };

        // Update processed UIDLs
        let updated_uidls: Vec<&str> = new_processed.iter().map(|s| s.as_str()).collect();
        let uidls_str = updated_uidls.join(",");
        {
            let db = self.db.lock().expect("db lock");
            db.execute(
                "UPDATE email_sources SET processed_uidls = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                params![source.id, uidls_str],
            )?;
        }

        Ok(EmailSyncResult {
            source_id: source.id,
            fetched_count,
            imported_count,
            jobs: all_jobs,
        })
    }

    fn extract_attachments(
        &self,
        mail: &ParsedMail,
        keywords: &[String],
        tmp_files: &mut Vec<PathBuf>,
    ) {
        if mail.subparts.is_empty() {
            // Leaf part - check if it's an attachment
            let ct = mail.ctype.mimetype.to_lowercase();
            let is_attachment = mail
                .get_content_disposition()
                .params
                .get("filename")
                .is_some()
                || ct.starts_with("application/")
                || ct.starts_with("image/");

            if is_attachment {
                let filename = mail
                    .get_content_disposition()
                    .params
                    .get("filename")
                    .cloned()
                    .unwrap_or_else(|| "attachment".into());

                // Filter by keywords if specified
                if !keywords.is_empty() {
                    let fname_lower = filename.to_lowercase();
                    if !keywords.iter().any(|kw| fname_lower.contains(kw)) {
                        return;
                    }
                }

                // Only process PDF and image files
                let ext = std::path::Path::new(&filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default()
                    .to_lowercase();
                if !matches!(
                    ext.as_str(),
                    "pdf" | "png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif"
                ) {
                    return;
                }

                if let Ok(body) = mail.get_body_raw() {
                    let tmp_path = self.raw_dir.join(format!(
                        "email_{}_{}",
                        Utc::now().timestamp_millis(),
                        filename
                    ));
                    if std::fs::write(&tmp_path, &body).is_ok() {
                        info!("Extracted email attachment: {filename}");
                        tmp_files.push(tmp_path);
                    }
                }
            }
        } else {
            // Recurse into subparts
            for subpart in &mail.subparts {
                self.extract_attachments(subpart, keywords, tmp_files);
            }
        }
    }
}
