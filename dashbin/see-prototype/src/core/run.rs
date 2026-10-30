//! Running a statement, as lemmesee's rounds ran them: `psql` in round 1's
//! container on this machine (round 1c's `see.run_sql`), or on a database's
//! own host through ssh in a read-only session with half a second a
//! statement (round 1k's `live.remote`). A row is its cells as text, a cell
//! the database left empty is `None`.

use std::io::Write;
use std::process::{Command, Stdio};

pub type Rows = Vec<Vec<Option<String>>>;

const NULL: &str = "<<null>>";

pub enum Runner {
    /// `docker exec <container> psql -U <user> -d <database>` on this machine
    Docker { container: String, user: String, database: String },
    /// `ssh <host> docker exec <container> psql`, read only
    Ssh { host: String, key: Option<String>, container: String, user: String, database: String },
}

fn rows_of(out: &str) -> Rows {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|v| if v == NULL || v == "NULL" { None } else { Some(v.to_string()) }).collect())
        .collect()
}

fn last_line(err: &str) -> String {
    err.trim().lines().last().unwrap_or("error").chars().take(300).collect()
}

impl Runner {
    pub fn run(&self, sql: &str) -> (Rows, Option<String>) {
        let out = match self {
            Runner::Docker { container, user, database } => Command::new("docker")
                .args(["exec", "-i", "-e", "PGOPTIONS=-c statement_timeout=5000", container, "psql", "-U", user, "-d", database, "-At", "-F", "\t", "-P", &format!("null={NULL}"), "-v", "ON_ERROR_STOP=1", "-c", sql])
                .output(),
            Runner::Ssh { host, key, container, user, database } => {
                let opts = "-c default_transaction_read_only=on -c statement_timeout=500 -c lock_timeout=1000";
                // every word quoted for the remote shell: an unquoted <<null>> is a here-document there
                let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
                let line = format!(
                    "docker exec -i -e {} {} psql -U {} -d {} -At -F $'\\t' -P {} -v ON_ERROR_STOP=1 -f -",
                    q(&format!("PGOPTIONS={opts}")), q(container), q(user), q(database), q(&format!("null={NULL}"))
                );
                let mut cmd = Command::new(if cfg!(windows) { "ssh.exe" } else { "ssh" });
                if let Some(k) = key {
                    cmd.args(["-i", k]);
                }
                cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", host, &line]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
                match cmd.spawn() {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            let _ = stdin.write_all(sql.as_bytes());
                        }
                        child.wait_with_output()
                    }
                    Err(e) => Err(e),
                }
            }
        };
        match out {
            Ok(o) if o.status.success() => (rows_of(&String::from_utf8_lossy(&o.stdout)), None),
            Ok(o) => (vec![], Some(last_line(&String::from_utf8_lossy(&o.stderr)))),
            Err(e) => (vec![], Some(e.to_string())),
        }
    }
}
