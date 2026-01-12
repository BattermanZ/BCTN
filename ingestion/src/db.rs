use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};

pub(crate) fn open_readonly(db: &Path) -> Result<Connection> {
    Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open sqlite db (read-only) {db:?}"))
}

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema.sql"))?;
    Ok(())
}

pub(crate) fn ensure_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema_indexes.sql"))?;
    Ok(())
}

pub(crate) fn open_rw_with_schema(db: &Path) -> Result<Connection> {
    let conn = Connection::open(db).with_context(|| format!("open sqlite db {db:?}"))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    ensure_schema(&conn)?;
    ensure_indexes(&conn)?;
    Ok(conn)
}
