use anyhow::{Context, Result, bail};
use argon2::{Argon2, PasswordHasher};
use rand::RngCore;
use sqlx::{Pool, Sqlite};
use time::OffsetDateTime;

fn now_ts() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn hash_password(password: &str) -> Result<String> {
    use argon2::password_hash::SaltString;
    let salt = {
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        SaltString::encode_b64(&bytes).map_err(|e| anyhow::anyhow!("salt: {e}"))?
    };
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash password: {e}"))?
        .to_string();
    Ok(hash)
}

pub(crate) async fn bootstrap_admin(
    db: &Pool<Sqlite>,
    username: &str,
    password: &str,
) -> Result<()> {
    if username.trim().is_empty() {
        bail!("username required");
    }
    if password.len() < 8 {
        bail!("password too short");
    }

    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_user")
        .fetch_one(db)
        .await
        .context("count users")?;
    if total_users != 0 {
        bail!("bootstrap only allowed when there are 0 users");
    }

    let now = now_ts();
    let password_hash = hash_password(password)?;

    let mut tx = db.begin().await?;
    let user_id = sqlx::query(
        "INSERT INTO app_user (username, password_hash, is_admin, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)",
    )
    .bind(username.trim())
    .bind(password_hash)
    .bind(now)
    .execute(&mut *tx)
    .await
    .context("insert admin user")?
    .last_insert_rowid();

    sqlx::query("INSERT OR IGNORE INTO user_settings (user_id) VALUES (?1)")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .ok();

    tx.commit().await?;
    Ok(())
}
