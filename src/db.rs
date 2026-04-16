use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio_rusqlite::{Connection, params};
use tracing::info;

pub struct HostkeyDB {
    connection: Connection,

    #[allow(dead_code)]
    path: PathBuf,
}

impl HostkeyDB {
    pub async fn new(state_dir: String) -> Result<Self> {
        let dir_ref = &state_dir;
        let joined_path = Path::new(dir_ref).join(Path::new("hostkeys.db"));
        let connection = Connection::open(joined_path.as_path()).await?;
        Self::prepare(&connection).await?;
        Ok(Self {
            path: joined_path,
            connection,
        })
    }

    async fn prepare(db_conn: &Connection) -> Result<()> {
        let () = db_conn
            .call(|conn| {
                conn.execute(
                    r#"CREATE TABLE IF NOT EXISTS hostkeys (
                    hostname TEXT UNIQUE,
                    KEY TEXT,
                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );"#,
                    (),
                )?;
                tokio_rusqlite::Result::Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn authenticate(&self, hostname: &str, public_key: String) -> bool {
        match self.get_key(hostname).await {
            None => {
                info!(hostname, "HostkeyDB entry does not exist for this host");
                false
            }
            Some(stored_key) => match stored_key {
                None => {
                    info!(hostname, "Known host connecting for the first time");
                    self.set_key(hostname, public_key).await
                }
                Some(previous_key) => {
                    let success = public_key == previous_key;
                    info!(hostname, success, "HostkeyDB entry exists for host");
                    success
                }
            },
        }
    }

    // Outer option: row with this hostname exists
    // inner option: there is a pubkey for it in the db
    async fn get_key(&self, hostname: &str) -> Option<Option<String>> {
        let hostname_owned = hostname.to_string();
        let result = self
            .connection
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT key FROM hostkeys WHERE hostname = ?1")?;
                let mut rows = stmt.query([&hostname_owned])?;
                match rows.next()? {
                    Some(row) => {
                        let key: Option<String> = row.get(0)?;
                        tokio_rusqlite::Result::Ok(Some(key))
                    }
                    None => Ok(None),
                }
            })
            .await;

        match result {
            Ok(pubkey) => pubkey,
            Err(err) => {
                info!(hostname, ?err, "Error checking hostkeyDB");
                None
            }
        }
    }

    pub async fn init_hosts(&self, hostnames: impl Iterator<Item = impl AsRef<str>>) -> Result<()> {
        let hostnames: Vec<String> = hostnames.map(|h| h.as_ref().to_string()).collect();
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                {
                    let mut stmt =
                        tx.prepare("INSERT OR IGNORE INTO hostkeys (hostname) VALUES (?1)")?;
                    for hostname in &hostnames {
                        stmt.execute([hostname])?;
                    }
                }
                tx.commit()?;
                tokio_rusqlite::Result::Ok(())
            })
            .await?;
        Ok(())
    }

    async fn set_key(&self, hostname: &str, public_key: String) -> bool {
        let hostname_owned = hostname.to_string();
        let result = self
            .connection
            .call(move |conn| {
                conn.execute(
                    "UPDATE hostkeys SET key = ?1, updated_at = CURRENT_TIMESTAMP \
                     WHERE hostname = ?2",
                    params![public_key, hostname_owned],
                )?;
                tokio_rusqlite::Result::Ok(())
            })
            .await;

        match result {
            Ok(()) => true,
            Err(err) => {
                info!(hostname, ?err, "Error updating hostkey");
                false
            }
        }
    }
}
