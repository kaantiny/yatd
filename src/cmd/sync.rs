//! Peer-to-peer project sync via magic-wormhole.
//!
//! Both peers open the same project, exchange Loro version vectors,
//! compute deltas containing only the ops the other side lacks, then
//! exchange and import those deltas.  The result is that both docs
//! converge to the same state without sending duplicate operations.

use std::borrow::Cow;
use std::path::Path;

use anyhow::{bail, Context, Result};
use loro::{ExportMode, VersionVector};
use magic_wormhole::{AppConfig, AppID, Code, MailboxConnection, Wormhole};
use serde::{Deserialize, Serialize};

use crate::db;

/// Custom AppID scoping our wormhole traffic away from other protocols.
const APP_ID: &str = "td.sync.v1";

/// Number of random words in the generated wormhole code.
const CODE_WORD_COUNT: usize = 2;

/// Handshake message exchanged before the delta payload.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "mode")]
enum SyncHandshake {
    Sync {
        /// Human-readable project name.
        project_name: String,
        /// Stable identity (ULID stored in the doc's root meta map).
        project_id: String,
        /// Serialised version vector so the peer can compute a minimal delta.
        #[serde(with = "vv_serde")]
        version_vector: VersionVector,
    },
    Bootstrap {
        /// Serialised version vector so the peer can compute a minimal delta.
        #[serde(with = "vv_serde")]
        version_vector: VersionVector,
    },
}

/// Serde adapter for `VersionVector` using its postcard `encode()`/`decode()`.
mod vv_serde {
    use loro::VersionVector;
    use serde::{self, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(vv: &VersionVector, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_bytes(&vv.encode())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<VersionVector, D::Error> {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(de)?;
        VersionVector::decode(&bytes).map_err(serde::de::Error::custom)
    }
}

/// Outcome of a sync exchange, returned by [`exchange`].
pub struct SyncReport {
    pub sent_bytes: usize,
    pub received_bytes: usize,
    pub imported: bool,
}

pub fn wormhole_config() -> AppConfig<serde_json::Value> {
    AppConfig {
        id: AppID::new(APP_ID),
        rendezvous_url: Cow::Borrowed(magic_wormhole::rendezvous::DEFAULT_RENDEZVOUS_SERVER),
        app_version: serde_json::json!({"v": 1}),
    }
}

/// Run the sync protocol over an already-established wormhole.
///
/// Both sides call this concurrently.  The protocol is symmetric: each
/// peer sends its version vector, receives the other's, computes a
/// minimal delta, sends it, receives the peer's delta, and imports it.
pub async fn exchange(store: &db::Store, mut wormhole: Wormhole) -> Result<SyncReport> {
    let my_handshake = SyncHandshake::Sync {
        project_name: store.project_name().to_string(),
        project_id: read_project_id(store)?,
        version_vector: store.doc().oplog_vv(),
    };

    // --- Phase 1: exchange handshakes ---
    wormhole
        .send_json(&my_handshake)
        .await
        .context("failed to send handshake")?;

    let their_handshake: SyncHandshake = wormhole
        .receive_json::<SyncHandshake>()
        .await
        .context("failed to receive handshake")?
        .context("peer sent invalid handshake JSON")?;

    let their_vv = match &their_handshake {
        SyncHandshake::Sync {
            project_name,
            project_id,
            version_vector,
        } => {
            let (my_project_name, my_project_id) = match &my_handshake {
                SyncHandshake::Sync {
                    project_name,
                    project_id,
                    ..
                } => (project_name, project_id),
                SyncHandshake::Bootstrap { .. } => unreachable!("sync exchange always uses Sync"),
            };
            if my_project_id != project_id {
                let _ = wormhole.close().await;
                bail!(
                    "project identity mismatch: local '{}' ({}) vs peer '{}' ({}). If this is the same logical project, remove the accidentally initted local copy and bootstrap with 'td sync' instead of running 'td init' on both machines",
                    my_project_name,
                    my_project_id,
                    project_name,
                    project_id,
                );
            }
            version_vector
        }
        SyncHandshake::Bootstrap { version_vector } => version_vector,
    };

    // --- Phase 2: compute and exchange deltas ---
    let my_delta = store
        .doc()
        .export(ExportMode::updates(their_vv))
        .context("failed to export delta for peer")?;

    wormhole
        .send(my_delta.clone())
        .await
        .context("failed to send delta")?;

    let their_delta = wormhole
        .receive()
        .await
        .context("failed to receive delta from peer")?;

    wormhole.close().await.context("failed to close wormhole")?;

    // --- Phase 3: import the peer's delta locally ---
    let imported = if !their_delta.is_empty() {
        store
            .doc()
            .import(&their_delta)
            .context("failed to import peer delta")?;
        store.doc().commit();
        store.save_raw_delta(&their_delta)?;
        true
    } else {
        false
    };

    Ok(SyncReport {
        sent_bytes: my_delta.len(),
        received_bytes: their_delta.len(),
        imported,
    })
}

pub fn run(root: &Path, code: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    rt.block_on(run_async(root, code, json))
}

async fn run_async(root: &Path, code: Option<&str>, json: bool) -> Result<()> {
    let maybe_store = db::try_open(root)?;
    let c = crate::color::stderr_theme();

    let wormhole = connect_wormhole(code, json, c).await?;

    let (store, report) = if let Some(store) = maybe_store {
        if !json {
            eprintln!("{}wormhole:{} connected, syncing...", c.blue, c.reset);
        }
        let report = exchange(&store, wormhole).await?;
        (store, report)
    } else {
        if !json {
            eprintln!(
                "{}wormhole:{} connected, bootstrapping from peer...",
                c.blue, c.reset
            );
        }
        bootstrap_exchange(root, wormhole).await?
    };

    print_sync_report(&store, &report, json, c)?;

    Ok(())
}

async fn bootstrap_exchange(
    root: &Path,
    mut wormhole: Wormhole,
) -> Result<(db::Store, SyncReport)> {
    wormhole
        .send_json(&SyncHandshake::Bootstrap {
            version_vector: VersionVector::default(),
        })
        .await
        .context("failed to send bootstrap handshake")?;

    let their_handshake: SyncHandshake = wormhole
        .receive_json::<SyncHandshake>()
        .await
        .context("failed to receive handshake")?
        .context("peer sent invalid handshake JSON")?;

    let project_name = match their_handshake {
        SyncHandshake::Sync { project_name, .. } => project_name,
        SyncHandshake::Bootstrap { .. } => {
            let _ = wormhole.close().await;
            bail!(
                "both peers are in bootstrap mode. Run 'td init <project>' on one machine first, then run 'td sync' on the other"
            );
        }
    };

    wormhole
        .send(Vec::new())
        .await
        .context("failed to send bootstrap delta")?;

    let their_delta = wormhole
        .receive()
        .await
        .context("failed to receive bootstrap delta from peer")?;

    wormhole.close().await.context("failed to close wormhole")?;

    if their_delta.is_empty() {
        bail!("peer sent empty bootstrap delta");
    }

    let store = db::bootstrap_sync(root, &project_name, &their_delta)?;
    let report = SyncReport {
        sent_bytes: 0,
        received_bytes: their_delta.len(),
        imported: true,
    };
    Ok((store, report))
}

async fn connect_wormhole(
    code: Option<&str>,
    json: bool,
    c: &crate::color::Theme,
) -> Result<Wormhole> {
    match code {
        None => {
            let mailbox = MailboxConnection::create(wormhole_config(), CODE_WORD_COUNT)
                .await
                .context("failed to create wormhole mailbox")?;

            let code = mailbox.code().clone();
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({"code": code.to_string()}))?
                );
            } else {
                eprintln!("{}wormhole:{} run on the other machine:\n", c.blue, c.reset);
                eprintln!("  td sync {}{}{}\n", c.bold, code, c.reset);
                eprintln!("waiting for peer...");
            }

            Wormhole::connect(mailbox)
                .await
                .context("wormhole key exchange failed")
        }
        Some(raw) => {
            let code: Code = raw.parse().context("invalid wormhole code")?;
            let mailbox = MailboxConnection::connect(wormhole_config(), code, false)
                .await
                .context("failed to connect to wormhole mailbox")?;

            if !json {
                eprintln!("{}wormhole:{} connecting...", c.blue, c.reset);
            }

            Wormhole::connect(mailbox)
                .await
                .context("wormhole key exchange failed")
        }
    }
}

fn print_sync_report(
    store: &db::Store,
    report: &SyncReport,
    json: bool,
    c: &crate::color::Theme,
) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "synced": true,
                "project": store.project_name(),
                "sent_bytes": report.sent_bytes,
                "received_bytes": report.received_bytes,
            }))?
        );
    } else {
        eprintln!(
            "{}synced:{} {} (sent {} bytes, received {} bytes)",
            c.green,
            c.reset,
            store.project_name(),
            report.sent_bytes,
            report.received_bytes,
        );
        if report.imported {
            eprintln!("{}info:{} imported peer changes", c.blue, c.reset);
        } else {
            eprintln!("{}info:{} peer had no new changes", c.blue, c.reset);
        }
    }
    Ok(())
}

/// Read the stable project identity from the doc's root meta map.
fn read_project_id(store: &db::Store) -> Result<String> {
    let root = serde_json::to_value(store.doc().get_deep_value())?;
    root.get("meta")
        .and_then(|m| m.get("project_id"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("missing meta.project_id in project doc"))
}
