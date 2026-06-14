import { useMemo, useState } from "react";
import type { MouseEvent, PointerEvent } from "react";
import { Clock, Copy, Download, FolderOpen, Import, Play, Plus, Save, Trash2 } from "lucide-react";
import { api, emptyRequest } from "./api";
import type {
  AuthConfig,
  HermesEnvironment,
  HermesRequest,
  HermesResponse,
  HistoryEntry,
  HttpMethod,
  ImportKind,
  KeyValueRow,
  TreeNode,
  WorkspaceConfig
} from "./types";

const methods: HttpMethod[] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

const slugify = (value: string) => value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");

const requestPathFor = (name: string, group: string) => {
  const requestSlug = slugify(name) || "request";
  const groupSlug = group
    .split("/")
    .map((part) => slugify(part))
    .filter(Boolean)
    .join("/");
  return `collections/${groupSlug ? `${groupSlug}/` : ""}${requestSlug}.yaml`;
};

const normalizeGroupInput = (group: string) =>
  group
    .split("/")
    .map((part) => slugify(part))
    .filter(Boolean)
    .join("/");

const groupFromPath = (path: string) => {
  const parts = path.replace(/\\/g, "/").split("/");
  if (parts[0] !== "collections" || parts.length <= 2) return "";
  return parts.slice(1, -1).join("/");
};

const groupFromFolderPath = (path: string) => {
  const parts = path.replace(/\\/g, "/").split("/");
  if (parts[0] !== "collections") return path;
  return parts.slice(1).join("/");
};

const groupNameFromPath = (group: string) => {
  const parts = group.split("/").filter(Boolean);
  return parts[parts.length - 1] || group;
};

const replaceGroupName = (group: string, name: string) => {
  const parts = group.split("/").filter(Boolean);
  parts[parts.length - 1] = slugify(name) || "group";
  return parts.join("/");
};

const collectRequestPaths = (nodes: TreeNode[]): string[] =>
  nodes.flatMap((node) => (node.kind === "request" ? [node.path] : collectRequestPaths(node.children)));

const collectRequests = (nodes: TreeNode[]): Extract<TreeNode, { kind: "request" }>[] =>
  nodes.flatMap((node) => (node.kind === "request" ? [node] : collectRequests(node.children)));

const uniqueRequestPath = (basePath: string, nodes: TreeNode[], ignorePath = "") => {
  const existing = new Set(collectRequestPaths(nodes).filter((path) => path !== ignorePath));
  if (!existing.has(basePath)) return basePath;

  const match = basePath.match(/^(.*?)(\.ya?ml)$/);
  const stem = match?.[1] || basePath;
  const extension = match?.[2] || ".yaml";
  for (let index = 2; index < 1000; index += 1) {
    const candidate = `${stem}-${index}${extension}`;
    if (!existing.has(candidate)) return candidate;
  }
  return `${stem}-${crypto.randomUUID().slice(0, 8)}${extension}`;
};

type ContextMenuState =
  | { x: number; y: number; target: "root" }
  | { x: number; y: number; target: "folder"; group: string }
  | { x: number; y: number; target: "request"; path: string; group: string };

interface GroupDialogState {
  parentGroup: string;
  value: string;
}

type RenameDialogState =
  | { kind: "folder"; group: string; value: string }
  | { kind: "request"; path: string; group: string; value: string };

interface PointerDragState {
  path: string;
  label: string;
  startX: number;
  startY: number;
  x: number;
  y: number;
  active: boolean;
}

export function App() {
  const [workspacePath, setWorkspacePath] = useState("");
  const [workspace, setWorkspace] = useState<WorkspaceConfig | null>(null);
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [selectedPath, setSelectedPath] = useState("");
  const [request, setRequest] = useState<HermesRequest>(emptyRequest);
  const [requestGroup, setRequestGroup] = useState("");
  const [environments, setEnvironments] = useState<HermesEnvironment[]>([]);
  const [environmentId, setEnvironmentId] = useState<string>("");
  const [response, setResponse] = useState<HermesResponse | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [activeTab, setActiveTab] = useState<"params" | "headers" | "body" | "auth">("params");
  const [error, setError] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importKind, setImportKind] = useState<ImportKind>("curl");
  const [importPayload, setImportPayload] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [groupDialog, setGroupDialog] = useState<GroupDialogState | null>(null);
  const [deleteGroupDialog, setDeleteGroupDialog] = useState("");
  const [renameDialog, setRenameDialog] = useState<RenameDialogState | null>(null);
  const [openGroups, setOpenGroups] = useState<Record<string, boolean>>({});
  const [pointerDrag, setPointerDrag] = useState<PointerDragState | null>(null);

  const selectedEnvironment = environments.find((env) => env.id === environmentId);

  async function refreshWorkspace() {
    if (!workspacePath) return;
    setError("");
    try {
      const [nextWorkspace, nextTree, nextEnvironments, nextHistory] = await Promise.all([
        api.readWorkspaceConfig(workspacePath),
        api.listWorkspaceTree(workspacePath),
        api.listEnvironments(workspacePath),
        api.listHistory(workspacePath)
      ]);
      setWorkspace(nextWorkspace);
      setTree(nextTree);
      setEnvironments(nextEnvironments);
      setEnvironmentId((current) => (nextEnvironments.some((env) => env.id === current) ? current : ""));
    } catch (err) {
      setError(toMessage(err));
    }
  }

  async function createWorkspace() {
    const path = workspacePath.trim();
    if (!path) {
      setError("Enter a workspace folder path first.");
      return;
    }
    setBusy(true);
    try {
      const created = await api.createWorkspace(path, "Hermes Workspace");
      setWorkspace(created);
      await refreshWorkspace();
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function openWorkspace() {
    const path = workspacePath.trim();
    if (!path) return;
    setBusy(true);
    try {
      const opened = await api.openWorkspace(path);
      setWorkspace(opened);
      await refreshWorkspace();
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function selectRequest(path: string) {
    if (!workspacePath) return;
    setSelectedPath(path);
    setRequestGroup(groupFromPath(path));
    setError("");
    try {
      setRequest(await api.readRequest(workspacePath, path));
    } catch (err) {
      setError(toMessage(err));
    }
  }

  async function saveRequest() {
    if (!workspacePath) {
      setError("Open a workspace before saving requests.");
      return;
    }
    setBusy(true);
    try {
      const shouldMove = selectedPath && groupFromPath(selectedPath) !== requestGroup.trim();
      const desiredPath = requestPathFor(request.name, requestGroup);
      const path = selectedPath && !shouldMove ? selectedPath : uniqueRequestPath(desiredPath, tree, selectedPath);
      const writtenPath = await api.writeRequest(workspacePath, path, request);
      if (selectedPath && selectedPath !== writtenPath) {
        await api.deleteRequest(workspacePath, selectedPath);
      }
      setSelectedPath(writtenPath);
      setRequestGroup(groupFromPath(writtenPath));
      await refreshWorkspace();
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function sendRequest() {
    if (!workspacePath) {
      setError("Open a workspace before sending requests.");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const result = await api.sendRequest({ workspace_path: workspacePath, request, environment_id: environmentId || undefined });
      setResponse(result);
      setHistory(await api.listHistory(workspacePath));
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function duplicateSelected() {
    await duplicateRequestAt(selectedPath);
  }

  async function duplicateRequestAt(path: string) {
    if (!workspacePath || !path) return;
    setBusy(true);
    try {
      const newPath = await api.duplicateRequest(workspacePath, path);
      await refreshWorkspace();
      await selectRequest(newPath);
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function newRequestInGroup(group = "") {
    setSelectedPath("");
    setRequestGroup(group);
    setRequest(emptyRequest());
    setResponse(null);
    setError("");
    setContextMenu(null);
  }

  function openCreateGroupDialog(parentGroup = "") {
    if (!workspacePath) {
      setError("Open a workspace before creating groups.");
      return;
    }
    setContextMenu(null);
    setGroupDialog({
      parentGroup,
      value: parentGroup ? `${parentGroup}/new-group` : "new-group"
    });
  }

  async function createGroup() {
    if (!workspacePath || !groupDialog) return;
    const group = normalizeGroupInput(groupDialog.value);
    if (!group) return;
    setBusy(true);
    setContextMenu(null);
    try {
      await api.createGroup(workspacePath, group);
      setGroupDialog(null);
      await refreshWorkspace();
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function requestDeleteGroup(group: string) {
    setContextMenu(null);
    setDeleteGroupDialog(group);
  }

  function openRenameGroupDialog(group: string) {
    setContextMenu(null);
    setRenameDialog({ kind: "folder", group, value: groupNameFromPath(group) });
  }

  function openRenameRequestDialog(path: string) {
    setContextMenu(null);
    const node = collectRequests(tree).find((requestNode) => requestNode.path === path);
    setRenameDialog({ kind: "request", path, group: groupFromPath(path), value: node?.name || request.name });
  }

  async function deleteGroup() {
    const group = deleteGroupDialog;
    if (!workspacePath || !group) return;
    setBusy(true);
    try {
      await api.deleteGroup(workspacePath, group, true);
      if (selectedPath.startsWith(`collections/${group}/`)) {
        setSelectedPath("");
        setRequestGroup("");
        setRequest(emptyRequest());
        setResponse(null);
      }
      setDeleteGroupDialog("");
      await refreshWorkspace();
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function renameGroup() {
    if (!workspacePath || !renameDialog || renameDialog.kind !== "folder") return;
    const newGroup = replaceGroupName(renameDialog.group, renameDialog.value);
    if (!newGroup) return;

    setBusy(true);
    try {
      await api.renameGroup(workspacePath, renameDialog.group, newGroup);
      const oldPrefix = `collections/${renameDialog.group}/`;
      const newPrefix = `collections/${newGroup}/`;
      const nextSelectedPath = selectedPath.startsWith(oldPrefix)
        ? selectedPath.replace(oldPrefix, newPrefix)
        : selectedPath;
      setOpenGroups((current) => {
        const next: Record<string, boolean> = {};
        for (const [group, open] of Object.entries(current)) {
          if (group === renameDialog.group) {
            next[newGroup] = open;
          } else if (group.startsWith(`${renameDialog.group}/`)) {
            next[group.replace(`${renameDialog.group}/`, `${newGroup}/`)] = open;
          } else {
            next[group] = open;
          }
        }
        next[newGroup] = next[newGroup] ?? true;
        return next;
      });
      setRenameDialog(null);
      await refreshWorkspace();
      if (nextSelectedPath !== selectedPath) {
        await selectRequest(nextSelectedPath);
      }
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function renameRequest() {
    if (!workspacePath || !renameDialog || renameDialog.kind !== "request") return;
    const name = renameDialog.value.trim();
    if (!name) return;

    setBusy(true);
    try {
      const requestToRename = await api.readRequest(workspacePath, renameDialog.path);
      const renamedRequest = { ...requestToRename, name };
      const desiredPath = requestPathFor(name, renameDialog.group);
      const newPath = desiredPath === renameDialog.path
        ? renameDialog.path
        : uniqueRequestPath(desiredPath, tree, renameDialog.path);
      await api.writeRequest(workspacePath, newPath, renamedRequest);
      if (newPath !== renameDialog.path) {
        await api.deleteRequest(workspacePath, renameDialog.path);
      }
      setRenameDialog(null);
      await refreshWorkspace();
      if (selectedPath === renameDialog.path) {
        await selectRequest(newPath);
      }
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function applyRename() {
    if (renameDialog?.kind === "folder") {
      await renameGroup();
    } else if (renameDialog?.kind === "request") {
      await renameRequest();
    }
  }

  function isGroupOpen(group: string) {
    return openGroups[group] ?? true;
  }

  function toggleGroup(group: string) {
    setOpenGroups((current) => ({ ...current, [group]: !(current[group] ?? true) }));
  }

  async function moveRequestToGroup(requestPath: string, targetGroup: string) {
    if (!workspacePath || !requestPath) return;
    const currentGroup = groupFromPath(requestPath);
    if (currentGroup === targetGroup) return;

    setBusy(true);
    setContextMenu(null);
    try {
      const movedRequest = await api.readRequest(workspacePath, requestPath);
      const desiredPath = requestPathFor(movedRequest.name, targetGroup);
      const newPath = uniqueRequestPath(desiredPath, tree, requestPath);
      await api.writeRequest(workspacePath, newPath, movedRequest);
      await api.deleteRequest(workspacePath, requestPath);
      await refreshWorkspace();
      await selectRequest(newPath);
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function startPointerDrag(event: PointerEvent, path: string, label: string) {
    if (event.button !== 0) return;
    event.preventDefault();
    setPointerDrag({
      path,
      label,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
      active: false
    });
  }

  function movePointerDrag(event: PointerEvent) {
    if (!pointerDrag) return;
    const distance = Math.hypot(event.clientX - pointerDrag.startX, event.clientY - pointerDrag.startY);
    setPointerDrag({
      ...pointerDrag,
      x: event.clientX,
      y: event.clientY,
      active: pointerDrag.active || distance > 4
    });
  }

  function finishPointerDrag(event: PointerEvent) {
    if (!pointerDrag) return;
    const drag = pointerDrag;
    setPointerDrag(null);

    if (!drag.active) {
      void selectRequest(drag.path);
      return;
    }

    const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-drop-group]");
    if (!target) return;
    void moveRequestToGroup(drag.path, target.dataset.dropGroup || "");
  }

  async function deleteSelected() {
    await deleteRequestAt(selectedPath);
  }

  async function deleteRequestAt(path: string) {
    if (!workspacePath || !path) return;
    setBusy(true);
    try {
      await api.deleteRequest(workspacePath, path);
      if (path === selectedPath) {
        setSelectedPath("");
        setRequestGroup("");
        setRequest(emptyRequest());
      }
      await refreshWorkspace();
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function runImport() {
    if (!workspacePath) {
      setError("Open a workspace before importing.");
      return;
    }
    setBusy(true);
    try {
      const result =
        importKind === "curl"
          ? await api.importCurl(workspacePath, importPayload)
          : importKind === "postman"
            ? await api.importPostmanCollection(workspacePath, importPayload)
            : await api.importOpenApi(workspacePath, importPayload);
      setShowImport(false);
      setImportPayload("");
      await refreshWorkspace();
      if (result.warnings.length) setError(result.warnings.join("\n"));
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const formattedBody = useMemo(() => formatBody(response?.body, response?.content_type), [response]);

  return (
    <main
      className="app-shell"
      onClick={() => setContextMenu(null)}
      onContextMenu={(event) => event.preventDefault()}
      onPointerMove={movePointerDrag}
      onPointerUp={finishPointerDrag}
      onPointerCancel={() => setPointerDrag(null)}
    >
      <aside className="sidebar">
        <div className="brand">
          <div>
            <h1>Hermes</h1>
            <p>Local API client</p>
          </div>
          <button title="New request" onClick={() => { setSelectedPath(""); setRequestGroup(""); setRequest(emptyRequest()); }}>
            <Plus size={18} />
          </button>
        </div>

        <div className="sidebar-section workspace-section">
          <label className="field-label">Workspace folder</label>
          <div className="workspace-row">
            <input value={workspacePath} onChange={(event) => setWorkspacePath(event.target.value)} placeholder="F:\\Code\\apis" />
            <button title="Open workspace" onClick={openWorkspace} disabled={busy}>
              <FolderOpen size={17} />
            </button>
          </div>
          <button className="full-button" onClick={createWorkspace} disabled={busy}>Create workspace</button>
        </div>

        <section
          className="sidebar-section requests-section"
          onContextMenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
            setContextMenu({ x: event.clientX, y: event.clientY, target: "root" });
          }}
        >
          <div className="section-heading">
            <span>{workspace?.name || "Requests"}</span>
            <button title="Import" onClick={() => setShowImport(true)}><Import size={16} /></button>
          </div>
          <div
            className={pointerDrag?.active ? "collection-zone dragging" : "collection-zone"}
          >
            <div className="collection-scroll">
              <Tree
                nodes={tree}
                selectedPath={selectedPath}
                isDragging={Boolean(pointerDrag?.active)}
                isGroupOpen={isGroupOpen}
                onToggleGroup={toggleGroup}
                onSelect={(path) => void selectRequest(path)}
                onPointerDragStart={startPointerDrag}
                onContextMenu={(event, menu) => {
                  event.preventDefault();
                  event.stopPropagation();
                  setContextMenu({ x: event.clientX, y: event.clientY, ...menu });
                }}
              />
            </div>
            <div
              className="drop-target root-drop-target"
              data-drop-group=""
            >
              Drop here to move to collection root
            </div>
          </div>
        </section>

        <section className="sidebar-section history-section">
          <div className="section-heading">
            <span><Clock size={14} /> History</span>
          </div>
          {history.length === 0 ? <p className="muted">No requests sent yet.</p> : history.slice(0, 8).map((entry) => (
            <button key={entry.id} className="history-item" onClick={() => { setRequest(entry.request); setResponse(entry.response || null); }}>
              <span>{entry.method} {entry.status || "ERR"}</span>
              <small>{entry.request_name}</small>
            </button>
          ))}
        </section>
      </aside>

      <section className="main-pane">
        <header className="topbar">
          <input className="request-name" value={request.name} onChange={(event) => setRequest({ ...request, name: event.target.value })} />
          <input
            className="group-input"
            value={requestGroup}
            onChange={(event) => setRequestGroup(event.target.value)}
            placeholder="Group e.g. auth/admin"
            title="Request group folder"
          />
          <select
            value={environmentId}
            onChange={(event) => setEnvironmentId(event.target.value)}
            disabled={environments.length === 0}
          >
            <option value="">No environment</option>
            {environments.map((env) => <option key={env.id} value={env.id}>{env.name}</option>)}
          </select>
          <button title="Duplicate request" onClick={duplicateSelected} disabled={!selectedPath || busy}><Copy size={17} /></button>
          <button title="Delete request" onClick={deleteSelected} disabled={!selectedPath || busy}><Trash2 size={17} /></button>
          <button title="Save request" onClick={saveRequest} disabled={busy}><Save size={17} /></button>
          <button className="send-button" onClick={sendRequest} disabled={busy}><Play size={17} /> Send</button>
        </header>

        {error && <pre className="error-box">{error}</pre>}

        <div className="url-row">
          <select
            className={`method-select method-${request.method.toLowerCase()}`}
            value={request.method}
            onChange={(event) => setRequest({ ...request, method: event.target.value as HttpMethod })}
          >
            {methods.map((method) => <option key={method} value={method} className={`method-${method.toLowerCase()}`}>{method}</option>)}
          </select>
          <input value={request.url} onChange={(event) => setRequest({ ...request, url: event.target.value })} placeholder="{{baseUrl}}/users" />
        </div>

        <div className="editor-grid">
          <section className="editor-panel">
            <nav className="tabs">
              {(["params", "headers", "body", "auth"] as const).map((tab) => (
                <button key={tab} className={activeTab === tab ? "active" : ""} onClick={() => setActiveTab(tab)}>{tab}</button>
              ))}
            </nav>
            {activeTab === "params" && (
              <KeyValueEditor rows={request.params} onChange={(params) => setRequest({ ...request, params })} emptyLabel="Add query parameter" />
            )}
            {activeTab === "headers" && (
              <KeyValueEditor rows={request.headers} onChange={(headers) => setRequest({ ...request, headers })} emptyLabel="Add header" />
            )}
            {activeTab === "body" && <BodyEditor request={request} onChange={setRequest} />}
            {activeTab === "auth" && <AuthEditor auth={request.auth} onChange={(auth) => setRequest({ ...request, auth })} />}
          </section>

          <section className="response-panel">
            <div className="response-meta">
              {response ? (
                <>
                  <strong className={response.status >= 400 ? "bad-status" : "good-status"}>{response.status} {response.status_text}</strong>
                  <span>{response.elapsed_ms} ms</span>
                  <span>{response.size_bytes} B</span>
                  <button title="Download body" onClick={() => downloadText(response.body, `${request.name || "response"}.txt`)}>
                    <Download size={16} />
                  </button>
                </>
              ) : <span className="muted">Send a request to inspect the response.</span>}
            </div>
            {response && (
              <>
                <details>
                  <summary>Headers</summary>
                  <KeyValueTable rows={response.headers} />
                </details>
                <pre className="response-body">{formattedBody}</pre>
              </>
            )}
          </section>
        </div>

        <footer className="env-preview">
          <strong>Environment</strong>
          <span>{selectedEnvironment ? `${selectedEnvironment.values.filter((row) => row.enabled).length} active values` : "No environment selected"}</span>
        </footer>
      </section>

      {showImport && (
        <div className="modal-backdrop">
          <div className="modal">
            <header>
              <h2>Import collection</h2>
              <button onClick={() => setShowImport(false)}>Close</button>
            </header>
            <select value={importKind} onChange={(event) => setImportKind(event.target.value as ImportKind)}>
              <option value="curl">cURL</option>
              <option value="postman">Postman collection JSON</option>
              <option value="openapi">OpenAPI 3.x JSON/YAML</option>
            </select>
            <textarea value={importPayload} onChange={(event) => setImportPayload(event.target.value)} placeholder="Paste import payload here" />
            <button className="send-button" onClick={runImport} disabled={busy || !importPayload.trim()}>Import</button>
          </div>
        </div>
      )}

      {contextMenu && (
        <div
          className="context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          {contextMenu.target === "root" && (
            <>
              <button onClick={() => newRequestInGroup("")}>New request</button>
              <button onClick={() => openCreateGroupDialog("")}>New group</button>
            </>
          )}
          {contextMenu.target === "folder" && (
            <>
              <button onClick={() => openRenameGroupDialog(contextMenu.group)}>Rename group</button>
              <button onClick={() => newRequestInGroup(contextMenu.group)}>New request in group</button>
              <button onClick={() => openCreateGroupDialog(contextMenu.group)}>New subgroup</button>
              <button onClick={() => requestDeleteGroup(contextMenu.group)}>Delete group</button>
            </>
          )}
          {contextMenu.target === "request" && (
            <>
              <button onClick={() => { void selectRequest(contextMenu.path); setContextMenu(null); }}>Open</button>
              <button onClick={() => openRenameRequestDialog(contextMenu.path)}>Rename request</button>
              <button onClick={() => { void duplicateRequestAt(contextMenu.path); setContextMenu(null); }}>Duplicate</button>
              <button onClick={() => { void deleteRequestAt(contextMenu.path); setContextMenu(null); }}>Delete request</button>
            </>
          )}
        </div>
      )}

      {groupDialog && (
        <div className="modal-backdrop">
          <div className="modal small-modal" onClick={(event) => event.stopPropagation()}>
            <header>
              <h2>New group</h2>
              <button onClick={() => setGroupDialog(null)}>Close</button>
            </header>
            <label className="field-label">Group path</label>
            <input
              autoFocus
              value={groupDialog.value}
              onChange={(event) => setGroupDialog({ ...groupDialog, value: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === "Enter") void createGroup();
                if (event.key === "Escape") setGroupDialog(null);
              }}
              placeholder={groupDialog.parentGroup ? `${groupDialog.parentGroup}/new-group` : "new-group"}
            />
            <p className="muted">Use slashes for nested groups, for example auth/admin.</p>
            <div className="modal-actions">
              <button onClick={() => setGroupDialog(null)}>Cancel</button>
              <button className="send-button" onClick={() => void createGroup()} disabled={busy || !normalizeGroupInput(groupDialog.value)}>
                Create group
              </button>
            </div>
          </div>
        </div>
      )}

      {renameDialog && (
        <div className="modal-backdrop">
          <div className="modal small-modal" onClick={(event) => event.stopPropagation()}>
            <header>
              <h2>{renameDialog.kind === "folder" ? "Rename group" : "Rename request"}</h2>
              <button onClick={() => setRenameDialog(null)}>Close</button>
            </header>
            <label className="field-label">New name</label>
            <input
              autoFocus
              value={renameDialog.value}
              onChange={(event) => setRenameDialog({ ...renameDialog, value: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === "Enter") void applyRename();
                if (event.key === "Escape") setRenameDialog(null);
              }}
              placeholder={renameDialog.kind === "folder" ? "group-name" : "Request name"}
            />
            {renameDialog.kind === "folder" && (
              <p className="muted">
                Group folders use URL-friendly names, for example "Admin APIs" becomes admin-apis.
              </p>
            )}
            <div className="modal-actions">
              <button onClick={() => setRenameDialog(null)}>Cancel</button>
              <button
                className="send-button"
                onClick={() => void applyRename()}
                disabled={busy || (renameDialog.kind === "folder" ? !slugify(renameDialog.value) : !renameDialog.value.trim())}
              >
                Rename
              </button>
            </div>
          </div>
        </div>
      )}

      {deleteGroupDialog && (
        <div className="modal-backdrop">
          <div className="modal small-modal" onClick={(event) => event.stopPropagation()}>
            <header>
              <h2>Delete group</h2>
              <button onClick={() => setDeleteGroupDialog("")}>Close</button>
            </header>
            <p>
              Delete <strong>{deleteGroupDialog}</strong> and all contained requests and subgroups?
            </p>
            <p className="muted">This action cannot be undone.</p>
            <div className="modal-actions">
              <button onClick={() => setDeleteGroupDialog("")}>Cancel</button>
              <button className="danger-button" onClick={() => void deleteGroup()} disabled={busy}>
                Delete group
              </button>
            </div>
          </div>
        </div>
      )}

      {pointerDrag?.active && (
        <div className="drag-ghost" style={{ left: pointerDrag.x + 12, top: pointerDrag.y + 12 }}>
          {pointerDrag.label}
        </div>
      )}
    </main>
  );
}

function Tree({
  nodes,
  selectedPath,
  isDragging,
  isGroupOpen,
  onToggleGroup,
  onSelect,
  onPointerDragStart,
  onContextMenu
}: {
  nodes: TreeNode[];
  selectedPath: string;
  isDragging: boolean;
  isGroupOpen: (group: string) => boolean;
  onToggleGroup: (group: string) => void;
  onSelect: (path: string) => void;
  onPointerDragStart: (event: PointerEvent, path: string, label: string) => void;
  onContextMenu: (
    event: MouseEvent,
    menu: { target: "folder"; group: string } | { target: "request"; path: string; group: string }
  ) => void;
}) {
  if (!nodes.length) return <p className="muted">No requests yet.</p>;
  return (
    <div className="tree">
      {nodes.map((node) => {
        if (node.kind === "folder") {
          const group = groupFromFolderPath(node.path);
          const open = isGroupOpen(group);
          return (
            <div key={node.path} className="group-node">
              <div
                className="group-heading"
                data-drop-group={group}
                onContextMenu={(event) => onContextMenu(event, { target: "folder", group })}
              >
                <button
                  className="tree-toggle"
                  title={open ? "Collapse group" : "Expand group"}
                  onClick={(event) => {
                    event.stopPropagation();
                    onToggleGroup(group);
                  }}
                >
                  {open ? "▾" : "▸"}
                </button>
                <span>{node.name}</span>
              </div>
              {open && (
                <>
                  <Tree
                    nodes={node.children}
                    selectedPath={selectedPath}
                    isDragging={isDragging}
                    isGroupOpen={isGroupOpen}
                    onToggleGroup={onToggleGroup}
                    onSelect={onSelect}
                    onPointerDragStart={onPointerDragStart}
                    onContextMenu={onContextMenu}
                  />
                </>
              )}
            </div>
          );
        }

        return (
          <div
            key={node.path}
            role="button"
            tabIndex={0}
            className={selectedPath === node.path ? "tree-item active" : "tree-item"}
            onClick={() => onSelect(node.path)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") onSelect(node.path);
            }}
            onPointerDown={(event) => onPointerDragStart(event, node.path, node.name)}
            onContextMenu={(event) => onContextMenu(event, { target: "request", path: node.path, group: groupFromPath(node.path) })}
          >
            <span className={`method-label method-${node.method.toLowerCase()}`}>{node.method}</span>{node.name}
          </div>
        );
      })}
    </div>
  );
}

function KeyValueEditor({ rows, onChange, emptyLabel }: { rows: KeyValueRow[]; onChange: (rows: KeyValueRow[]) => void; emptyLabel: string }) {
  const update = (index: number, patch: Partial<KeyValueRow>) => onChange(rows.map((row, idx) => idx === index ? { ...row, ...patch } : row));
  return (
    <div className="kv-editor">
      {rows.map((row, index) => (
        <div className="kv-row" key={row.id}>
          <input type="checkbox" checked={row.enabled} onChange={(event) => update(index, { enabled: event.target.checked })} />
          <input value={row.key} onChange={(event) => update(index, { key: event.target.value })} placeholder="Key" />
          <input value={row.value} onChange={(event) => update(index, { value: event.target.value })} placeholder="Value" />
          <button onClick={() => onChange(rows.filter((_, idx) => idx !== index))}>Remove</button>
        </div>
      ))}
      <button className="ghost-button" onClick={() => onChange([...rows, { id: crypto.randomUUID(), key: "", value: "", enabled: true }])}>{emptyLabel}</button>
    </div>
  );
}

function BodyEditor({ request, onChange }: { request: HermesRequest; onChange: (request: HermesRequest) => void }) {
  return (
    <div className="body-editor">
      <select value={request.body.kind} onChange={(event) => onChange({ ...request, body: defaultBody(event.target.value) })}>
        <option value="none">No body</option>
        <option value="json">JSON</option>
        <option value="text">Text</option>
        <option value="form">Form URL encoded</option>
        <option value="binary">Binary path</option>
      </select>
      <textarea
        disabled={request.body.kind === "none"}
        value={request.body.content || ""}
        onChange={(event) => onChange({ ...request, body: { ...request.body, content: event.target.value } })}
        placeholder={request.body.kind === "json" ? "{\n  \"name\": \"Hermes\"\n}" : "Request body"}
      />
    </div>
  );
}

function defaultBody(kind: string) {
  switch (kind) {
    case "json":
      return { kind: "json" as const, content: "{\n  \n}" };
    case "text":
      return { kind: "text" as const, content: "" };
    case "form":
      return { kind: "form" as const, content: "" };
    case "binary":
      return { kind: "binary" as const, content: "" };
    default:
      return { kind: "none" as const, content: "" };
  }
}

function AuthEditor({ auth, onChange }: { auth: AuthConfig; onChange: (auth: AuthConfig) => void }) {
  return (
    <div className="auth-editor">
      <select value={auth.kind} onChange={(event) => onChange(defaultAuth(event.target.value as AuthConfig["kind"]))}>
        <option value="none">No auth</option>
        <option value="basic">Basic</option>
        <option value="bearer">Bearer token</option>
        <option value="api_key">API key</option>
        <option value="oauth2_client_credentials">OAuth2 client credentials</option>
      </select>
      {auth.kind === "basic" && (
        <>
          <input value={auth.username} onChange={(event) => onChange({ ...auth, username: event.target.value })} placeholder="Username" />
          <input value={auth.password} onChange={(event) => onChange({ ...auth, password: event.target.value })} placeholder="Password or {{secret}}" type="password" />
        </>
      )}
      {auth.kind === "bearer" && <input value={auth.token} onChange={(event) => onChange({ ...auth, token: event.target.value })} placeholder="Token or {{secret}}" />}
      {auth.kind === "api_key" && (
        <>
          <select value={auth.placement} onChange={(event) => onChange({ ...auth, placement: event.target.value as "header" | "query" })}>
            <option value="header">Header</option>
            <option value="query">Query</option>
          </select>
          <input value={auth.name} onChange={(event) => onChange({ ...auth, name: event.target.value })} placeholder="Name" />
          <input value={auth.value} onChange={(event) => onChange({ ...auth, value: event.target.value })} placeholder="Value or {{secret}}" />
        </>
      )}
      {auth.kind === "oauth2_client_credentials" && (
        <>
          <input value={auth.token_url} onChange={(event) => onChange({ ...auth, token_url: event.target.value })} placeholder="Token URL" />
          <input value={auth.client_id} onChange={(event) => onChange({ ...auth, client_id: event.target.value })} placeholder="Client ID" />
          <input value={auth.client_secret} onChange={(event) => onChange({ ...auth, client_secret: event.target.value })} placeholder="Client secret or {{secret}}" type="password" />
          <input value={auth.scopes.join(" ")} onChange={(event) => onChange({ ...auth, scopes: event.target.value.split(/\s+/).filter(Boolean) })} placeholder="Scopes" />
        </>
      )}
    </div>
  );
}

function KeyValueTable({ rows }: { rows: KeyValueRow[] }) {
  return (
    <table>
      <tbody>
        {rows.map((row) => (
          <tr key={`${row.key}-${row.value}`}>
            <th>{row.key}</th>
            <td>{row.value}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function defaultAuth(kind: AuthConfig["kind"]): AuthConfig {
  switch (kind) {
    case "basic":
      return { kind, username: "", password: "" };
    case "bearer":
      return { kind, token: "" };
    case "api_key":
      return { kind, placement: "header", name: "x-api-key", value: "" };
    case "oauth2_client_credentials":
      return { kind, token_url: "", client_id: "", client_secret: "", scopes: [] };
    default:
      return { kind: "none" };
  }
}

export function formatBody(body = "", contentType = "") {
  if (contentType.includes("json") || body.trim().startsWith("{") || body.trim().startsWith("[")) {
    try {
      return JSON.stringify(JSON.parse(body), null, 2);
    } catch {
      return body;
    }
  }
  return body;
}

function downloadText(text: string, filename: string) {
  const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function toMessage(err: unknown) {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return JSON.stringify(err, null, 2);
}
