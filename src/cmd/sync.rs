//! Peer-to-peer project sync via magic-wormhole.
//!
//! Both peers open the same project, exchange Loro version vectors,
//! compute deltas containing only the ops the other side lacks, then
//! exchange and import those deltas.  The result is that both docs
//! converge to the same state without sending duplicate operations.
//!
//! When neither peer has a project selected, they exchange a `SyncAll`
//! handshake carrying a manifest of all their local projects.  Each side
//! computes the intersection by `project_id` and syncs every shared
//! project in one wormhole session.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use loro::{ExportMode, VersionVector};
use magic_wormhole::{AppConfig, AppID, Code, MailboxConnection, Wormhole};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::PROJECTS_DIR;

/// Custom AppID scoping our wormhole traffic away from other protocols.
const APP_ID: &str = "td.sync.v1";

/// Number of random words in the generated wormhole code.
const CODE_WORD_COUNT: usize = 2;

/// One entry in a [`SyncAll`] manifest: name, stable id, and current VV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub project_name: String,
    pub project_id: String,
    #[serde(with = "vv_serde")]
    pub version_vector: VersionVector,
}

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
    /// Sent when no project is selected locally.  Carries a manifest of every
    /// local project so the peer can compute the intersection.
    SyncAll { projects: Vec<ProjectEntry> },
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

/// Outcome of a single-project sync exchange, returned by [`exchange`] and
/// [`sync_all_exchange`].
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

/// Import a delta into a store and persist it.
///
/// Commits the document and saves the raw delta for storage. Returns true
/// if any changes were imported (delta was non-empty).
fn import_and_persist(store: &db::Store, delta: &[u8]) -> Result<bool> {
    if delta.is_empty() {
        return Ok(false);
    }
    store
        .doc()
        .import(delta)
        .context("failed to import delta")?;
    store.doc().commit();
    store.save_raw_delta(delta)?;
    Ok(true)
}

/// Run the sync protocol over an already-established wormhole.
///
/// Both sides call this concurrently.  The protocol is symmetric: each
/// peer sends its version vector, receives the other's, computes a
/// minimal delta, sends it, receives the peer's delta, and imports it.
///
/// If the peer sends `SyncAll` (no project selected on their end), the local
/// project is treated as a bootstrap source and a full delta is sent.
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
        .context(
            "peer sent incompatible handshake (are both sides running the same version of td?)",
        )?;

    // Determine the peer's version vector, validating project identity when
    // both sides have a project selected.
    let their_vv: VersionVector = match &their_handshake {
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
                SyncHandshake::Bootstrap { .. } | SyncHandshake::SyncAll { .. } => {
                    unreachable!("exchange always sends Sync")
                }
            };
            if my_project_id != project_id {
                let _ = wormhole.close().await;
                bail!(
                    "project identity mismatch: local '{}' ({}) vs peer '{}' ({}). If this is the same logical project, remove the accidentally initted local copy and bootstrap with 'td sync' instead of running 'td project init <project>' on both machines",
                    my_project_name,
                    my_project_id,
                    project_name,
                    project_id,
                );
            }
            version_vector.clone()
        }
        // Peer has no project; treat as a bootstrap request and export everything.
        SyncHandshake::Bootstrap { version_vector } => version_vector.clone(),
        SyncHandshake::SyncAll { projects } => {
            // Peer has no project selected but sent their manifest.
            // Find our project in their manifest to get their VV for us.
            let my_id = read_project_id(store)?;
            projects
                .iter()
                .find(|p| p.project_id == my_id)
                .map(|p| p.version_vector.clone())
                .unwrap_or_default()
        }
    };

    // --- Phase 2: compute and exchange deltas ---
    let my_delta = store
        .doc()
        .export(ExportMode::updates(&their_vv))
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
    let imported =
        import_and_persist(store, &their_delta).context("failed to import peer delta")?;

    Ok(SyncReport {
        sent_bytes: my_delta.len(),
        received_bytes: their_delta.len(),
        imported,
    })
}

/// Build a manifest of all local projects using the given data root.
///
/// Each entry carries the project name, stable id, and current version vector.
/// Using an explicit `data_root` (rather than `HOME`) makes this safe to call
/// from async contexts where `HOME` may vary between peers.
pub fn build_local_manifest(data_root: &Path) -> Result<Vec<ProjectEntry>> {
    let names = db::list_projects_in(data_root)?;
    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let store = db::Store::open(data_root, &name)?;
        let project_id = store.project_id()?;
        let version_vector = store.doc().oplog_vv();
        entries.push(ProjectEntry {
            project_name: name,
            project_id,
            version_vector,
        });
    }
    Ok(entries)
}

/// Run the SyncAll protocol over an already-established wormhole.
///
/// Sends a `SyncAll` handshake carrying `local_manifest`, then handles the
/// three possible responses:
///
/// - **`Sync`** — peer has a project selected; bootstrap that project from
///   them (identical semantics to the old single-peer bootstrap path).
/// - **`SyncAll`** — peer also has no project selected; compute the
///   intersection by `project_id`, sync each shared project in order, and
///   return one `(Store, SyncReport)` per synced project.  An empty
///   intersection is not an error; an empty `Vec` is returned.
/// - **`Bootstrap`** — peer is running an older td; bail with an upgrade hint.
///
/// `cwd` is only used when bootstrapping a brand-new project from a `Sync`
/// peer (to create the directory binding).  It is ignored in the `SyncAll`
/// case where all projects already exist locally.
pub async fn sync_all_exchange(
    cwd: &Path,
    data_root: &Path,
    local_manifest: Vec<ProjectEntry>,
    mut wormhole: Wormhole,
) -> Result<Vec<(db::Store, SyncReport)>> {
    // Send our manifest.  Clone so we can still use `local_manifest` below.
    wormhole
        .send_json(&SyncHandshake::SyncAll {
            projects: local_manifest.clone(),
        })
        .await
        .context("failed to send SyncAll handshake")?;

    let their_handshake: SyncHandshake = wormhole
        .receive_json::<SyncHandshake>()
        .await
        .context("failed to receive handshake")?
        .context(
            "peer sent incompatible handshake (are both sides running the same version of td?)",
        )?;

    match their_handshake {
        SyncHandshake::Sync {
            project_name,
            project_id,
            version_vector,
        } => {
            // Peer has a specific project selected.
            // Validate the peer's project name to prevent path traversal.
            db::validate_project_name(&project_name)?;
            let project_dir = data_root.join(PROJECTS_DIR).join(&project_name);

            if project_dir.exists() {
                // Project exists locally: open it, verify ID matches, then sync normally.
                let store = db::Store::open(data_root, &project_name).with_context(|| {
                    format!("failed to open existing project '{}'", project_name)
                })?;
                let my_id = read_project_id(&store)?;
                if my_id != project_id {
                    bail!(
                        "project identity mismatch: local '{}' ({}) vs peer '{}' ({}). \
                         Remove the accidentally initted local copy and bootstrap with 'td sync'",
                        project_name,
                        my_id,
                        project_name,
                        project_id
                    );
                }

                // Sync the existing project.
                let my_delta = store
                    .doc()
                    .export(ExportMode::updates(&version_vector))
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

                let imported = import_and_persist(&store, &their_delta)
                    .context("failed to import peer's delta")?;

                let report = SyncReport {
                    sent_bytes: my_delta.len(),
                    received_bytes: their_delta.len(),
                    imported,
                };
                Ok(vec![(store, report)])
            } else {
                // Project doesn't exist: bootstrap from peer.
                // The peer (who sent Sync) expects to send their full state and be done.
                wormhole
                    .send(Vec::new())
                    .await
                    .context("failed to send empty bootstrap delta")?;
                let their_delta = wormhole
                    .receive()
                    .await
                    .context("failed to receive bootstrap delta from peer")?;

                if their_delta.is_empty() {
                    bail!("peer sent empty bootstrap delta");
                }

                wormhole.close().await.context("failed to close wormhole")?;

                // Don't bind cwd when bootstrapping from SyncAll→Sync fallback —
                // the user didn't intend to bind their current directory to this project.
                let store =
                    db::bootstrap_sync_at(data_root, cwd, &project_name, &their_delta, false)?;

                let report = SyncReport {
                    sent_bytes: 0,
                    received_bytes: their_delta.len(),
                    imported: true,
                };
                Ok(vec![(store, report)])
            }
        }

        SyncHandshake::Bootstrap { .. } => {
            // Peer is running an older td that does not know about SyncAll.
            // The peer will have already failed when it received our SyncAll
            // handshake, so both sides are closing.
            let _ = wormhole.close().await;
            bail!(
                "peer is running an older version of td that does not support SyncAll. \
                 Upgrade td on both machines and try again"
            );
        }

        SyncHandshake::SyncAll {
            projects: their_projects,
        } => {
            // Both sides have no project selected.  Compute intersection by
            // project_id and sync each shared project.
            sync_shared_projects(data_root, local_manifest, their_projects, wormhole).await
        }
    }
}

/// Exchange deltas for every project whose `project_id` appears on both sides.
///
/// Both sides sort the intersection by `project_id` (deterministic ordering),
/// then for each project: send my delta, receive theirs, import.  This mirrors
/// the single-project [`exchange`] pattern — each side sends before it receives,
/// so the relay buffers the message and neither side blocks waiting for the
/// other to read first.
async fn sync_shared_projects(
    data_root: &Path,
    local_projects: Vec<ProjectEntry>,
    their_projects: Vec<ProjectEntry>,
    mut wormhole: Wormhole,
) -> Result<Vec<(db::Store, SyncReport)>> {
    // Wrap the body in an async block so we can ensure close() runs on all paths.
    let result: Result<Vec<(db::Store, SyncReport)>> = async {
        // Build a fast lookup from project_id → their entry.
        let their_by_id: HashMap<&str, &ProjectEntry> = their_projects
            .iter()
            .map(|p| (p.project_id.as_str(), p))
            .collect();

        // Collect the intersection, sorted by project_id for a deterministic wire
        // ordering that both peers independently agree on.
        let mut shared: Vec<(&ProjectEntry, &ProjectEntry)> = local_projects
            .iter()
            .filter_map(|mine| {
                their_by_id
                    .get(mine.project_id.as_str())
                    .map(|theirs| (mine, *theirs))
            })
            .collect();
        shared.sort_by(|a, b| a.0.project_id.cmp(&b.0.project_id));

        if shared.is_empty() {
            return Ok(vec![]);
        }

        // For each project: open store, export delta, send/receive, import.
        // Stores are opened one at a time to avoid exhausting file descriptors
        // when syncing many projects.
        let mut results: Vec<(db::Store, SyncReport)> = Vec::with_capacity(shared.len());
        for (mine, theirs) in shared {
            if mine.project_name != theirs.project_name {
                eprintln!(
                    "warning: project name mismatch for id {}: \
                     local '{}', peer '{}' — syncing by id",
                    mine.project_id, mine.project_name, theirs.project_name,
                );
            }
            let store = db::Store::open(data_root, &mine.project_name)
                .with_context(|| format!("failed to open project '{}'", mine.project_name))?;
            let my_delta = store
                .doc()
                .export(ExportMode::updates(&theirs.version_vector))
                .with_context(|| {
                    format!("failed to export delta for '{}'", store.project_name())
                })?;

            wormhole
                .send(my_delta.clone())
                .await
                .with_context(|| format!("failed to send delta for '{}'", store.project_name()))?;

            let their_delta = wormhole.receive().await.with_context(|| {
                format!("failed to receive delta for '{}'", store.project_name())
            })?;

            let imported = import_and_persist(&store, &their_delta).with_context(|| {
                format!("failed to import delta for '{}'", store.project_name())
            })?;

            results.push((
                store,
                SyncReport {
                    sent_bytes: my_delta.len(),
                    received_bytes: their_delta.len(),
                    imported,
                },
            ));
        }

        Ok(results)
    }
    .await;

    // Always close the wormhole, but don't let close errors mask the original result.
    let _ = wormhole.close().await;
    result
}

pub fn run(root: &Path, code: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    rt.block_on(run_async(root, code, json))
}

async fn run_async(root: &Path, code: Option<&str>, json: bool) -> Result<()> {
    let maybe_store = db::try_open(root)?;
    let c = crate::color::stderr_theme();

    let wormhole = connect_wormhole(code, json, c).await?;

    if let Some(store) = maybe_store {
        // A project is selected: single-project sync.
        if !json {
            eprintln!("{}wormhole:{} connected, syncing...", c.blue, c.reset);
        }
        let report = exchange(&store, wormhole).await?;
        print_sync_report(&store, &report, json, c)?;
    } else {
        // No project selected: enumerate local projects and attempt SyncAll.
        let data_root = db::data_root()?;
        let local_manifest = build_local_manifest(&data_root)?;

        if !json {
            if local_manifest.is_empty() {
                eprintln!(
                    "{}wormhole:{} connected, bootstrapping from peer...",
                    c.blue, c.reset
                );
            } else {
                eprintln!(
                    "{}wormhole:{} connected, syncing all shared projects...",
                    c.blue, c.reset
                );
            }
        }

        let results = sync_all_exchange(root, &data_root, local_manifest, wormhole).await?;

        if results.is_empty() {
            if json {
                println!(
                    "{}",
                    serde_json::to_string(
                        &serde_json::json!({"synced": false, "reason": "no_shared_projects"})
                    )?
                );
            } else {
                eprintln!("{}info:{} no shared projects to sync", c.blue, c.reset);
            }
        } else {
            // Iterate by value to drop each Store (and its file handle) immediately
            // after printing, rather than holding all handles until the loop ends.
            for (store, report) in results {
                print_sync_report(&store, &report, json, c)?;
            }
        }
    }

    Ok(())
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
    store.project_id()
}
