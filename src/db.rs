use anyhow::Result;
use sqlite;
use std::path::{Path, PathBuf};
use tracing::info;

pub struct HostkeyDB {
    connection: sqlite::ConnectionThreadSafe,

    #[allow(dead_code)]
    path: PathBuf,
}

impl HostkeyDB {
    pub fn new(state_dir: String) -> Result<Self> {
        let dir_ref = &state_dir;
        let joined_path = Path::new(dir_ref).join(Path::new("hostkeys.db"));
        let connection = sqlite::Connection::open_thread_safe(joined_path.as_path())?;
        Self::prepare(&connection)?;
        Ok(Self {
            path: joined_path,
            connection,
        })
    }

    fn prepare(db_conn: &sqlite::ConnectionThreadSafe) -> Result<()> {
        db_conn.execute(
            r#"CREATE TABLE IF NOT EXISTS hostkeys (
                hostname TEXT UNIQUE,
                KEY TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );"#,
        )?;
        Ok(())
    }

    pub fn authenticate(&self, hostname: &str, public_key: String) -> bool {
        match self.get_key(hostname) {
            None => {
                info!(hostname, "HostkeyDB entry does not exist for this host");
                false
            }
            Some(stored_key) => match stored_key {
                None => {
                    info!(hostname, "Known host connecting for the first time");
                    self.set_key(hostname, public_key)
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
    fn get_key(&self, hostname: &str) -> Option<Option<String>> {
        let mut pubkey: Option<Option<String>> = None;
        // TODO: maybe check if for some reason there are many rows even though the table is unique?
        let result = self.connection.iterate(
            format!("SELECT key FROM hostkeys WHERE hostname = '{}'", hostname),
            |row| {
                // row is an array of (column_name, value) tuples
                let key = row[0].1.map(|s| s.to_string());
                pubkey = Some(key);
                true
            },
        );

        match result {
            Err(err) => {
                info!(hostname, ?err, "Error checking hostkeyDB");
            }
            _ => {}
        }
        return pubkey;
    }

    pub fn init_hosts(&self, hostnames: impl Iterator<Item = impl AsRef<str>>) -> Result<()> {
        for hostname in hostnames {
            self.connection.execute(format!(
                "INSERT OR IGNORE INTO hostkeys (hostname) VALUES ('{}');",
                hostname.as_ref()
            ))?;
        }
        Ok(())
    }

    fn set_key(&self, hostname: &str, public_key: String) -> bool {
        match self.connection.execute(format!(
            "UPDATE hostkeys SET key = '{}', updated_at = CURRENT_TIMESTAMP WHERE hostname = '{}';",
            public_key, hostname
        )) {
            Ok(()) => true,
            Err(err) => {
                info!(hostname, ?err, "Error updating hostkey");
                false
            }
        }
    }
}
