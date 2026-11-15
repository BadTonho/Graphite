//! Virtual file systems (`MainThreadFileSystem` / `ExtHostFileSystem`).
//!
//! An extension registers a provider for a URI scheme. Graphite reads a virtual file from the
//! provider into a local mirror (`<profile>/virtual-fs/<scheme>/<authority>/<path>`) and opens that
//! mirror, so the editor, language servers and syntax highlighting work unchanged. Documents under
//! the mirror are identified to extensions by their virtual URI, and saving a mirrored file writes
//! it back through the provider. Registered schemes also appear as folders in the explorer.

use super::*;
use base64::Engine;
use graphite_core_api::DocumentUri;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::RwLock;

/// Replies to file system calls carry ids in `FS_BASE..FS_END`.
pub(super) const FS_BASE: u64 = 15_000_000;
const FS_END: u64 = 16_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// `FileSystemProviderCapabilities.Readonly`.
const READONLY_CAPABILITY: u64 = 1 << 11;

pub(super) fn is_fs_reply(id: u64) -> bool {
    (FS_BASE..FS_END).contains(&id)
}

/// The folder virtual files are mirrored into, and the same folder with links resolved (a path
/// reaches the app in either form).
#[derive(Clone)]
struct MirrorRoot {
    raw: PathBuf,
    resolved: PathBuf,
}

static MIRROR_ROOT: RwLock<Option<MirrorRoot>> = RwLock::new(None);

/// Sets the folder virtual files are mirrored into.
pub(crate) fn set_mirror_root(root: PathBuf) {
    let _ = std::fs::create_dir_all(&root);
    let resolved = root.canonicalize().unwrap_or_else(|_| root.clone());
    if let Ok(mut current) = MIRROR_ROOT.write() {
        *current = Some(MirrorRoot {
            raw: root,
            resolved,
        });
    }
}

fn mirror_root() -> Option<PathBuf> {
    MIRROR_ROOT
        .read()
        .ok()
        .and_then(|root| root.as_ref().map(|root| root.raw.clone()))
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for ch in segment.chars() {
        if ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '.' | '_' | '-' | ' ' | '~' | '@' | '+' | ',' | '=' | '(' | ')'
            )
            || !ch.is_ascii()
        {
            out.push(ch);
        } else {
            let mut buffer = [0u8; 4];
            for byte in ch.encode_utf8(&mut buffer).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    // Windows does not keep a trailing dot or space in a name.
    match out.pop() {
        Some(last @ ('.' | ' ')) => out.push_str(&format!("%{:02X}", last as u8)),
        Some(last) => out.push(last),
        None => {}
    }
    out
}

fn decode_segment(segment: &str) -> String {
    super::webviews::percent_decode(segment)
}

/// The parts of a virtual URI that identify a file: scheme, authority and absolute path.
fn uri_parts(uri: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = uri.split_once(':')?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (authority, path) = match rest.strip_prefix("//") {
        Some(after) => match after.find('/') {
            Some(index) => (after[..index].to_owned(), after[index..].to_owned()),
            None => (after.to_owned(), "/".to_owned()),
        },
        None => (String::new(), rest.to_owned()),
    };
    Some((
        scheme.to_ascii_lowercase(),
        super::webviews::percent_decode(&authority),
        super::webviews::percent_decode(&path),
    ))
}

fn encode_uri_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Builds the URI string of a scheme, authority and absolute path.
fn build_uri(scheme: &str, authority: &str, path: &str) -> String {
    let path = encode_uri_path(path);
    if authority.is_empty() {
        format!("{scheme}:{path}")
    } else {
        format!("{scheme}://{authority}{path}")
    }
}

/// The local mirror of a virtual URI under `root`. `None` for a path that would leave the mirror.
fn mirror_path_in(root: &Path, uri: &str) -> Option<PathBuf> {
    let (scheme, authority, path) = uri_parts(uri)?;
    let mut local = root.join(encode_segment(&scheme));
    // An empty authority and the authority `_` stay distinct.
    local.push(if authority.is_empty() {
        "_".to_owned()
    } else {
        encode_segment(&authority).replace('_', "%5F")
    });
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        if segment == "." || segment == ".." || segment.contains(['\\', '\0']) {
            return None;
        }
        local.push(encode_segment(segment));
    }
    Some(local)
}

/// The virtual URI a local mirror path stands for.
fn uri_for_mirror(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = relative.components().map(|part| part.as_os_str().to_str());
    let scheme = decode_segment(parts.next()??);
    let authority = decode_segment(parts.next()??);
    let authority = if authority == "_" && relative.components().nth(1)?.as_os_str() == "_" {
        String::new()
    } else {
        authority
    };
    let mut remote = String::new();
    for part in parts {
        remote.push('/');
        remote.push_str(&decode_segment(part?));
    }
    if remote.is_empty() {
        remote.push('/');
    }
    Some(build_uri(&scheme, &authority, &remote))
}

/// The local mirror file of a virtual URI.
pub(crate) fn mirror_path(uri: &DocumentUri) -> Option<PathBuf> {
    mirror_path_in(&mirror_root()?, uri.as_str())
}

/// The virtual URI of a local file when it is a mirror of one.
pub(crate) fn uri_for_path(path: &Path) -> Option<DocumentUri> {
    let roots = {
        let guard = MIRROR_ROOT.read().ok()?;
        let root = guard.as_ref()?;
        [root.raw.clone(), root.resolved.clone()]
    };
    roots.iter().find_map(|root| {
        path.starts_with(root)
            .then(|| uri_for_mirror(root, path))
            .flatten()
            .and_then(|uri| DocumentUri::parse(&uri).ok())
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VirtualEntry {
    pub name: String,
    pub dir: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct VirtualDirectory {
    pub entries: Option<Vec<VirtualEntry>>,
    pub expanded: bool,
    pub error: Option<String>,
}

/// One row of the explorer's virtual folders.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VirtualRow {
    pub uri: String,
    pub name: String,
    pub depth: usize,
    pub dir: bool,
    pub expanded: bool,
}

#[derive(Clone, Debug)]
struct Provider {
    scheme: String,
    readonly: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Op {
    /// Read a file, mirror it and open it; `reply` is the extension request waiting for the result.
    Open {
        uri: String,
        reply: Option<u64>,
    },
    List {
        uri: String,
    },
    Save {
        uri: String,
    },
    Other {
        what: &'static str,
        refresh: Option<String>,
    },
}

#[derive(Default)]
pub(super) struct VirtualFs {
    next_id: u64,
    next_watch: u64,
    providers: BTreeMap<u64, Provider>,
    pending: BTreeMap<u64, (Op, Instant)>,
    dirs: BTreeMap<String, VirtualDirectory>,
}

impl Drop for VirtualFs {
    fn drop(&mut self) {
        for provider in self.providers.values() {
            graphite_core_api::unregister_virtual_scheme(&provider.scheme);
        }
    }
}

impl VirtualFs {
    pub(super) fn busy(&self) -> bool {
        !self.pending.is_empty()
    }

    pub(super) fn expire(&mut self, now: Instant) {
        self.pending.retain(|_, (_, deadline)| *deadline > now);
    }

    fn handle_for_scheme(&self, scheme: &str) -> Option<u64> {
        self.providers
            .iter()
            .find(|(_, provider)| provider.scheme == scheme)
            .map(|(handle, _)| *handle)
    }

    pub(crate) fn is_readonly(&self, scheme: &str) -> bool {
        self.providers
            .values()
            .any(|provider| provider.scheme == scheme && provider.readonly)
    }

    fn root_uri(scheme: &str) -> String {
        format!("{scheme}:/")
    }

    fn next_id(&mut self) -> u64 {
        let id = FS_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (FS_END - FS_BASE);
        id
    }

    /// Rows of every provider root and of the folders that are expanded.
    pub(crate) fn rows(&self) -> Vec<VirtualRow> {
        let mut schemes: Vec<&str> = self
            .providers
            .values()
            .map(|provider| provider.scheme.as_str())
            .collect();
        schemes.sort_unstable();
        schemes.dedup();
        let mut rows = Vec::new();
        for scheme in schemes {
            let root = Self::root_uri(scheme);
            self.push_rows(&mut rows, &root, scheme, 0);
        }
        rows
    }

    fn push_rows(&self, rows: &mut Vec<VirtualRow>, uri: &str, name: &str, depth: usize) {
        let state = self.dirs.get(uri);
        let expanded = state.is_some_and(|state| state.expanded);
        rows.push(VirtualRow {
            uri: uri.to_owned(),
            name: name.to_owned(),
            depth,
            dir: true,
            expanded,
        });
        let Some(entries) = state
            .filter(|_| expanded)
            .and_then(|state| state.entries.as_ref())
        else {
            return;
        };
        let mut sorted: Vec<&VirtualEntry> = entries.iter().collect();
        sorted.sort_by(|a, b| {
            b.dir
                .cmp(&a.dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        for entry in sorted {
            let child = child_uri(uri, &entry.name);
            if entry.dir {
                self.push_rows(rows, &child, &entry.name, depth + 1);
            } else {
                rows.push(VirtualRow {
                    uri: child,
                    name: entry.name.clone(),
                    depth: depth + 1,
                    dir: false,
                    expanded: false,
                });
            }
        }
    }

    #[cfg(test)]
    pub(super) fn dirs_for_test(&mut self, uri: &str, expanded: bool) {
        self.dirs.entry(uri.to_owned()).or_default().expanded = expanded;
    }
}

/// The URI of `name` inside the folder `parent`.
fn child_uri(parent: &str, name: &str) -> String {
    let (scheme, authority, path) = uri_parts(parent).unwrap_or_default();
    let mut joined = path.trim_end_matches('/').to_owned();
    joined.push('/');
    joined.push_str(name);
    build_uri(&scheme, &authority, &joined)
}

fn uri_argument(uri: &str) -> Value {
    json!({ "$graphiteUri": uri })
}

fn parse_listing(result: &Value) -> Vec<VirtualEntry> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let name = entry.get(0)?.as_str()?.to_owned();
            let kind = entry.get(1)?.as_u64()?;
            (!name.is_empty() && !name.contains('/')).then_some(VirtualEntry {
                name,
                dir: kind & 2 != 0,
            })
        })
        .collect()
}

impl App {
    pub(super) fn handle_vsix_virtual_fs_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        match method {
            "$registerFileSystemProvider" => {
                let handle = args
                    .first()
                    .and_then(Value::as_u64)
                    .ok_or("Invalid file system provider handle")?;
                let scheme = args
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or("Invalid file system provider scheme")?
                    .to_ascii_lowercase();
                if !graphite_core_api::register_virtual_scheme(&scheme) {
                    return Err(format!(
                        "The scheme {scheme} cannot have a file system provider"
                    ));
                }
                let capabilities = args.get(2).and_then(Value::as_u64).unwrap_or(0);
                set_mirror_root(self.profile_root.join("virtual-fs"));
                runtime.vfs.providers.insert(
                    handle,
                    Provider {
                        scheme: scheme.clone(),
                        readonly: capabilities & READONLY_CAPABILITY != 0,
                    },
                );
                self.watch_virtual_root(runtime, handle, &scheme);
                Ok(Value::Null)
            }
            "$unregisterProvider" => {
                let handle = args.first().and_then(Value::as_u64);
                if let Some(provider) =
                    handle.and_then(|handle| runtime.vfs.providers.remove(&handle))
                {
                    let still_served = runtime
                        .vfs
                        .providers
                        .values()
                        .any(|other| other.scheme == provider.scheme);
                    if !still_served {
                        graphite_core_api::unregister_virtual_scheme(&provider.scheme);
                        let prefix = format!("{}:", provider.scheme);
                        runtime.vfs.dirs.retain(|uri, _| !uri.starts_with(&prefix));
                    }
                }
                Ok(Value::Null)
            }
            "$onFileSystemChange" => {
                // Listings may be stale: fetch the expanded folders again.
                let expanded: Vec<String> = runtime
                    .vfs
                    .dirs
                    .iter()
                    .filter(|(_, state)| state.expanded)
                    .map(|(uri, _)| uri.clone())
                    .collect();
                for uri in expanded {
                    let _ = self.request_virtual_listing(runtime, &uri);
                }
                Ok(Value::Null)
            }
            "$stat" | "$readdir" | "$readFile" | "$writeFile" | "$rm" | "$mkdir" | "$copy"
            | "$move" | "$watch" | "$unwatch" => {
                // Extensions reach their own providers inside the extension host. Anything that
                // arrives here targets a scheme Graphite does not serve.
                self.record_unsupported_vsix_call(runtime, "MainThreadFileSystem", method);
                Err(format!(
                    "Graphite does not serve this scheme through MainThreadFileSystem.{method}"
                ))
            }
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadFileSystem", method);
                Err(format!(
                    "Graphite does not support MainThreadFileSystem.{method}"
                ))
            }
        }
    }

    /// Sends a call to the provider that serves `uri` and remembers what to do with the result.
    fn virtual_call(
        &mut self,
        runtime: &mut Runtime,
        uri: &str,
        method: &str,
        extra: Vec<Value>,
        bytes: bool,
        op: Op,
    ) -> Result<u64> {
        let (scheme, _, _) = uri_parts(uri).ok_or("Invalid virtual URI")?;
        let handle = runtime
            .vfs
            .handle_for_scheme(&scheme)
            .ok_or_else(|| format!("No file system provider for {scheme}"))?;
        let id = runtime.vfs.next_id();
        let mut args = vec![json!(handle), uri_argument(uri)];
        args.extend(extra);
        runtime
            .vfs
            .pending
            .insert(id, (op, Instant::now() + REQUEST_TIMEOUT));
        let sent = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostFileSystem",
            "method": method,
            "args": args,
            "bytes": bytes,
        }));
        if let Err(error) = sent {
            runtime.vfs.pending.remove(&id);
            return Err(error);
        }
        Ok(id)
    }

    fn watch_virtual_root(&mut self, runtime: &mut Runtime, handle: u64, scheme: &str) {
        runtime.vfs.next_watch += 1;
        let session = runtime.vfs.next_watch;
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostFileSystem",
            "method": "$watch",
            "args": [handle, session, uri_argument(&VirtualFs::root_uri(scheme)),
                {"recursive": true, "excludes": []}],
        }));
    }

    pub(super) fn request_virtual_listing(
        &mut self,
        runtime: &mut Runtime,
        uri: &str,
    ) -> Result<u64> {
        self.virtual_call(
            runtime,
            uri,
            "$readdir",
            vec![],
            false,
            Op::List {
                uri: uri.to_owned(),
            },
        )
    }

    /// Reads a virtual file, mirrors it and opens it. `reply` is answered with the URI once open.
    pub(super) fn request_virtual_open(
        &mut self,
        runtime: &mut Runtime,
        uri: &str,
        reply: Option<u64>,
    ) -> Result<u64> {
        self.virtual_call(
            runtime,
            uri,
            "$readFile",
            vec![],
            true,
            Op::Open {
                uri: uri.to_owned(),
                reply,
            },
        )
    }

    /// Answers `MainThreadDocuments.$tryOpenDocument` for a virtual URI asynchronously. Returns
    /// true when the request was taken (its reply is sent later).
    pub(super) fn start_vsix_virtual_open(
        &mut self,
        runtime: &mut Runtime,
        id: u64,
        actor: &str,
        method: &str,
        args: &[Value],
    ) -> Result<bool> {
        if !matches!(
            (actor, method),
            ("MainThreadDocuments", "$tryOpenDocument")
                | ("MainThreadTextEditors", "$tryShowTextDocument")
        ) {
            return Ok(false);
        }
        let Some(uri) = args.first().and_then(Value::as_str) else {
            return Ok(false);
        };
        let virtual_uri = DocumentUri::parse(uri).ok().filter(DocumentUri::is_virtual);
        let Some(virtual_uri) = virtual_uri else {
            return Ok(false);
        };
        self.request_virtual_open(runtime, virtual_uri.as_str(), Some(id))?;
        Ok(true)
    }

    /// Writes a saved mirror file back through its provider.
    pub(crate) fn upload_virtual_document(&mut self, path: &Path) {
        let Some(uri) = uri_for_path(path) else {
            return;
        };
        let Some(mut runtime) = self.plugins.vsix.take() else {
            self.status = self.t("explorer.virtual.no_runtime").to_owned();
            return;
        };
        let result = std::fs::read(path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                self.virtual_call(
                    &mut runtime,
                    uri.as_str(),
                    "$writeFile",
                    vec![
                        json!({ "$graphiteBytes": encoded }),
                        json!({"create": true, "overwrite": true, "unlock": false, "atomic": false}),
                    ],
                    false,
                    Op::Save { uri: uri.as_str().to_owned() },
                )
            });
        if let Err(error) = result {
            self.status = error;
        }
        self.plugins.vsix = Some(runtime);
    }

    /// Deletes a virtual resource, then refreshes its folder.
    #[allow(dead_code)]
    pub(crate) fn request_virtual_delete(
        &mut self,
        runtime: &mut Runtime,
        uri: &str,
    ) -> Result<u64> {
        let parent = parent_uri(uri);
        self.virtual_call(
            runtime,
            uri,
            "$delete",
            vec![json!({"recursive": true, "useTrash": false, "atomic": false})],
            false,
            Op::Other {
                what: "delete",
                refresh: parent,
            },
        )
    }

    /// Renames a virtual resource within its provider, then refreshes its folder.
    #[allow(dead_code)]
    pub(crate) fn request_virtual_rename(
        &mut self,
        runtime: &mut Runtime,
        uri: &str,
        target: &str,
    ) -> Result<u64> {
        let parent = parent_uri(uri);
        self.virtual_call(
            runtime,
            uri,
            "$rename",
            vec![uri_argument(target), json!({"overwrite": false})],
            false,
            Op::Other {
                what: "rename",
                refresh: parent,
            },
        )
    }

    /// Reads the type, size and times of a virtual resource.
    #[allow(dead_code)]
    pub(crate) fn request_virtual_stat(&mut self, runtime: &mut Runtime, uri: &str) -> Result<u64> {
        self.virtual_call(
            runtime,
            uri,
            "$stat",
            vec![],
            false,
            Op::Other {
                what: "stat",
                refresh: None,
            },
        )
    }

    pub(super) fn finish_vsix_virtual_fs(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some((op, _)) = runtime.vfs.pending.remove(&id) else {
            return;
        };
        let error = event["error"].as_str().map(str::to_owned);
        match op {
            Op::List { uri } => {
                let state = runtime.vfs.dirs.entry(uri).or_default();
                match error {
                    Some(error) => {
                        state.error = Some(error.clone());
                        self.status = error;
                    }
                    None => {
                        state.error = None;
                        state.entries = Some(parse_listing(&event["result"]));
                    }
                }
            }
            Op::Open { uri, reply } => {
                let outcome = match error {
                    Some(error) => Err(error),
                    None => self.open_virtual_bytes(&uri, &event["result"]),
                };
                if let Err(error) = &outcome {
                    self.status = error.clone();
                }
                if let Some(request) = reply {
                    let message = match outcome {
                        Ok(()) => json!({"type": "reply", "id": request, "result": uri}),
                        Err(error) => json!({"type": "reply", "id": request, "error": error}),
                    };
                    let _ = runtime.session.send(message);
                }
            }
            Op::Save { uri } => match error {
                Some(error) => self.status = format!("{uri}: {error}"),
                None => self.status = format!("{} {uri}", self.t("explorer.virtual.saved")),
            },
            Op::Other { what, refresh } => {
                if let Some(error) = error {
                    self.status = format!("{what}: {error}");
                } else if let Some(parent) = refresh {
                    let _ = self.request_virtual_listing(runtime, &parent);
                }
            }
        }
    }

    /// Writes the bytes the provider returned to the mirror and opens the mirror.
    fn open_virtual_bytes(&mut self, uri: &str, result: &Value) -> std::result::Result<(), String> {
        let encoded = result["$bytes"]
            .as_str()
            .ok_or("The provider returned no file content")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| error.to_string())?;
        let document = DocumentUri::parse(uri).map_err(|error| error.to_string())?;
        let path = mirror_path(&document).ok_or("The resource cannot be mirrored")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        // A document with unsaved edits keeps them; only a clean one is refreshed.
        let dirty = self
            .documents
            .id_for_path(&path)
            .is_some_and(|id| self.documents[id].is_dirty());
        if !dirty {
            std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
        }
        self.open_file(path);
        Ok(())
    }

    /// The rows the explorer lists below the workspace tree.
    pub(crate) fn virtual_explorer_rows(&self) -> Vec<VirtualRow> {
        self.plugins
            .vsix
            .as_ref()
            .map(|runtime| runtime.vfs.rows())
            .unwrap_or_default()
    }

    /// Activates a virtual explorer row: folders expand or collapse, files open.
    pub(crate) fn activate_virtual_row(&mut self, row: &VirtualRow) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        if row.dir {
            let state = runtime.vfs.dirs.entry(row.uri.clone()).or_default();
            state.expanded = !state.expanded;
            let needs_listing = state.expanded && state.entries.is_none();
            if needs_listing {
                let _ = self.request_virtual_listing(&mut runtime, &row.uri);
            }
        } else if let Err(error) = self.request_virtual_open(&mut runtime, &row.uri, None) {
            self.status = error;
        }
        self.plugins.vsix = Some(runtime);
    }
}

fn parent_uri(uri: &str) -> Option<String> {
    let (scheme, authority, path) = uri_parts(uri)?;
    let trimmed = path.trim_end_matches('/');
    let parent = trimmed.rsplit_once('/').map(|(parent, _)| parent)?;
    Some(build_uri(
        &scheme,
        &authority,
        if parent.is_empty() { "/" } else { parent },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        std::env::temp_dir().join("graphite-vfs-test-root")
    }

    #[test]
    fn mirror_paths_round_trip_awkward_names() {
        let root = root();
        for uri in [
            "memfs:/docs/readme.md",
            "memfs:/a%20b/c%23d.txt",
            "memfs:/%E2%9C%93/%25done",
            "memfs://host/dir/file.rs",
            "memfs://_/x",
            "memfs:/trail./name",
            "memfs:/",
        ] {
            let local = mirror_path_in(&root, uri).unwrap();
            assert!(local.starts_with(&root), "{uri}");
            assert_eq!(uri_for_mirror(&root, &local).as_deref(), Some(uri), "{uri}");
        }
    }

    #[test]
    fn mirror_paths_never_leave_the_mirror() {
        let root = root();
        assert!(mirror_path_in(&root, "memfs:/a/../../etc/passwd").is_none());
        assert!(mirror_path_in(&root, "memfs:/a/./b").is_none());
        // An encoded separator is a separator once decoded, so it cannot hide a `..`.
        assert!(mirror_path_in(&root, "memfs:/a%2F..%2Fb").is_none());
        assert!(uri_for_mirror(&root, Path::new("/elsewhere/memfs/_/a")).is_none());
        let local = mirror_path_in(&root, "memfs:/CON:/x?y").unwrap();
        assert!(!local.to_string_lossy().contains('?'));
    }

    #[test]
    fn distinct_uris_keep_distinct_mirrors() {
        let root = root();
        let a = mirror_path_in(&root, "memfs:/x").unwrap();
        let b = mirror_path_in(&root, "memfs://_/x").unwrap();
        let c = mirror_path_in(&root, "other:/x").unwrap();
        assert!(a != b && a != c && b != c);
    }

    #[test]
    fn listings_parse_types_and_reject_bad_names() {
        let entries = parse_listing(&json!([
            ["a.txt", 1],
            ["src", 2],
            ["link", 66],
            ["bad/name", 1],
            ["", 1]
        ]));
        assert_eq!(
            entries,
            [
                VirtualEntry {
                    name: "a.txt".into(),
                    dir: false
                },
                VirtualEntry {
                    name: "src".into(),
                    dir: true
                },
                VirtualEntry {
                    name: "link".into(),
                    dir: true
                },
            ]
        );
    }

    #[test]
    fn rows_list_roots_then_expanded_folders_directories_first() {
        let mut fs = VirtualFs::default();
        fs.providers.insert(
            1,
            Provider {
                scheme: "memfs".into(),
                readonly: false,
            },
        );
        assert_eq!(fs.rows().len(), 1);
        assert!(!fs.rows()[0].expanded);
        fs.dirs.insert(
            "memfs:/".into(),
            VirtualDirectory {
                expanded: true,
                entries: Some(vec![
                    VirtualEntry {
                        name: "b.txt".into(),
                        dir: false,
                    },
                    VirtualEntry {
                        name: "Zed".into(),
                        dir: true,
                    },
                    VirtualEntry {
                        name: "a.txt".into(),
                        dir: false,
                    },
                ]),
                error: None,
            },
        );
        fs.dirs.insert(
            "memfs:/Zed".into(),
            VirtualDirectory {
                expanded: true,
                entries: Some(vec![VirtualEntry {
                    name: "in.rs".into(),
                    dir: false,
                }]),
                error: None,
            },
        );
        let rows: Vec<(String, usize)> = fs
            .rows()
            .into_iter()
            .map(|row| (row.uri, row.depth))
            .collect();
        assert_eq!(
            rows,
            [
                ("memfs:/".to_owned(), 0),
                ("memfs:/Zed".to_owned(), 1),
                ("memfs:/Zed/in.rs".to_owned(), 2),
                ("memfs:/a.txt".to_owned(), 1),
                ("memfs:/b.txt".to_owned(), 1),
            ]
        );
    }

    #[test]
    fn child_and_parent_uris_follow_the_path() {
        assert_eq!(child_uri("memfs:/", "a b.txt"), "memfs:/a%20b.txt");
        assert_eq!(child_uri("memfs://host/dir", "x"), "memfs://host/dir/x");
        assert_eq!(parent_uri("memfs:/dir/x").as_deref(), Some("memfs:/dir"));
        assert_eq!(parent_uri("memfs:/x").as_deref(), Some("memfs:/"));
    }

    #[test]
    fn pending_operations_expire() {
        let mut fs = VirtualFs::default();
        let id = fs.next_id();
        assert!(is_fs_reply(id));
        fs.pending.insert(
            id,
            (
                Op::List {
                    uri: "memfs:/".into(),
                },
                Instant::now(),
            ),
        );
        assert!(fs.busy());
        fs.expire(Instant::now() + Duration::from_secs(1));
        assert!(!fs.busy());
    }
}
