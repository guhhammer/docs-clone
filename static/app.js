const API_BASE = '/api';
const AUTOSAVE_DELAY = 1200;

const state = {
    owned: [],
    shared: [],
    current: null,
    userId: null,
    dirty: false,
    saving: false,
    filter: '',
};

const el = {
    userId: document.getElementById('userId'),
    newDocBtn: document.getElementById('newDocBtn'),
    emptyNewBtn: document.getElementById('emptyNewBtn'),
    uploadBtn: document.getElementById('uploadBtn'),
    emptyUploadBtn: document.getElementById('emptyUploadBtn'),
    fileUpload: document.getElementById('fileUpload'),
    searchInput: document.getElementById('searchInput'),
    ownedList: document.getElementById('ownedList'),
    sharedList: document.getElementById('sharedList'),
    ownedCount: document.getElementById('ownedCount'),
    sharedCount: document.getElementById('sharedCount'),
    editorContainer: document.getElementById('editorContainer'),
    emptyState: document.getElementById('emptyState'),
    docTitle: document.getElementById('docTitle'),
    docMeta: document.getElementById('docMeta'),
    accessBadge: document.getElementById('accessBadge'),
    editor: document.getElementById('editor'),
    saveBtn: document.getElementById('saveBtn'),
    deleteBtn: document.getElementById('deleteBtn'),
    closeBtn: document.getElementById('closeBtn'),
    shareBtn: document.getElementById('shareBtn'),
    videoBtn: document.getElementById('videoBtn'),
    saveStatus: document.getElementById('saveStatus'),
    wordCount: document.getElementById('wordCount'),
    blockStyle: document.getElementById('blockStyle'),
    tools: Array.from(document.querySelectorAll('.tool[data-command]')),
    sidebar: document.getElementById('sidebar'),
    sidebarToggle: document.getElementById('sidebarToggle'),
    toasts: document.getElementById('toasts'),
    dialogBackdrop: document.getElementById('dialogBackdrop'),
    dialogTitle: document.getElementById('dialogTitle'),
    dialogMessage: document.getElementById('dialogMessage'),
    dialogInput: document.getElementById('dialogInput'),
    dialogConfirm: document.getElementById('dialogConfirm'),
    dialogCancel: document.getElementById('dialogCancel'),
    shareBackdrop: document.getElementById('shareBackdrop'),
    shareForm: document.getElementById('shareForm'),
    shareUserId: document.getElementById('shareUserId'),
    sharePermission: document.getElementById('sharePermission'),
    shareList: document.getElementById('shareList'),
    shareClose: document.getElementById('shareClose'),
    videoBackdrop: document.getElementById('videoBackdrop'),
    videoUrl: document.getElementById('videoUrl'),
    videoError: document.getElementById('videoError'),
    videoConfirm: document.getElementById('videoConfirm'),
    videoCancel: document.getElementById('videoCancel'),
};

let autosaveTimer = null;
let savedRange = null;

document.addEventListener('DOMContentLoaded', () => {
    setupEventListeners();
    state.userId = currentUserId();
    loadDocuments();
});

function setupEventListeners() {
    el.newDocBtn.addEventListener('click', createNewDocument);
    el.emptyNewBtn.addEventListener('click', createNewDocument);
    el.uploadBtn.addEventListener('click', () => el.fileUpload.click());
    el.emptyUploadBtn.addEventListener('click', () => el.fileUpload.click());
    el.fileUpload.addEventListener('change', handleFileUpload);
    el.saveBtn.addEventListener('click', () => saveDocument({ silent: false }));
    el.closeBtn.addEventListener('click', closeEditor);
    el.deleteBtn.addEventListener('click', deleteCurrentDocument);
    el.shareBtn.addEventListener('click', openShareDialog);
    el.videoBtn.addEventListener('click', openVideoDialog);

    // Only reload when the identity actually changed: a blur-triggered `change`
    // event must not close the document the user just clicked on.
    el.userId.addEventListener('change', async () => {
        const next = currentUserId();
        if (next === state.userId) return;
        state.userId = next;
        await flushPendingSave();
        closeEditor();
        loadDocuments();
    });

    el.searchInput.addEventListener('input', () => {
        state.filter = el.searchInput.value.trim().toLowerCase();
        renderDocumentLists();
    });

    el.editor.addEventListener('input', () => {
        markDirty();
        updateWordCount();
    });
    el.docTitle.addEventListener('input', markDirty);

    el.editor.addEventListener('keyup', syncToolbarState);
    el.editor.addEventListener('mouseup', syncToolbarState);
    document.addEventListener('selectionchange', syncToolbarState);

    // Paste as plain text: the browser would otherwise inject arbitrary markup,
    // which the server strips anyway.
    el.editor.addEventListener('paste', (event) => {
        event.preventDefault();
        const text = (event.clipboardData || window.clipboardData).getData('text/plain');
        document.execCommand('insertText', false, text);
    });

    el.tools.forEach((tool) => {
        tool.addEventListener('mousedown', (event) => event.preventDefault());
        tool.addEventListener('click', () => {
            runCommand(tool.dataset.command, tool.dataset.value || null);
        });
    });

    el.blockStyle.addEventListener('change', () => {
        runCommand('formatBlock', el.blockStyle.value);
    });

    el.sidebarToggle.addEventListener('click', () => {
        const open = el.sidebar.classList.toggle('is-open');
        el.sidebarToggle.setAttribute('aria-expanded', String(open));
    });

    document.addEventListener('keydown', (event) => {
        const meta = event.ctrlKey || event.metaKey;
        if (meta && event.key.toLowerCase() === 's') {
            event.preventDefault();
            saveDocument({ silent: false });
        }
        if (event.key === 'Escape') {
            if (!el.dialogBackdrop.hidden) closeDialog(null);
            else if (!el.videoBackdrop.hidden) closeVideoDialog();
            else if (!el.shareBackdrop.hidden) closeShareDialog();
        }
    });

    el.shareForm.addEventListener('submit', grantAccess);
    el.shareClose.addEventListener('click', closeShareDialog);
    el.shareBackdrop.addEventListener('click', (event) => {
        if (event.target === el.shareBackdrop) closeShareDialog();
    });

    el.videoConfirm.addEventListener('click', insertVideo);
    el.videoCancel.addEventListener('click', closeVideoDialog);
    el.videoBackdrop.addEventListener('click', (event) => {
        if (event.target === el.videoBackdrop) closeVideoDialog();
    });
    el.videoUrl.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') {
            event.preventDefault();
            insertVideo();
        }
    });

    window.addEventListener('beforeunload', (event) => {
        if (state.dirty) {
            event.preventDefault();
            event.returnValue = '';
        }
    });
}

/* ---------- Data ---------- */

function currentUserId() {
    return el.userId.value.trim() || 'user1';
}

async function api(path, options = {}) {
    const headers = Object.assign({ 'X-User-Id': currentUserId() }, options.headers || {});
    const response = await fetch(`${API_BASE}${path}`, Object.assign({}, options, { headers }));

    if (!response.ok) {
        let message = `Request failed (${response.status})`;
        let body = null;
        try {
            body = await response.json();
            if (body && body.error) message = body.error;
        } catch (_) { /* keep default message */ }
        const error = new Error(message);
        error.status = response.status;
        error.body = body;
        throw error;
    }
    return response.status === 204 ? null : response.json();
}

async function loadDocuments() {
    el.ownedList.setAttribute('aria-busy', 'true');
    el.sharedList.setAttribute('aria-busy', 'true');
    try {
        const data = await api('/documents');
        state.owned = data.owned || [];
        state.shared = data.shared || [];
    } catch (error) {
        state.owned = [];
        state.shared = [];
        toast(`Could not load documents: ${error.message}`, 'error');
    } finally {
        el.ownedList.setAttribute('aria-busy', 'false');
        el.sharedList.setAttribute('aria-busy', 'false');
        renderDocumentLists();
    }
}

async function createNewDocument() {
    const title = await promptDialog({
        title: 'New document',
        message: 'Give your document a name. You can rename it any time.',
        value: 'Untitled document',
        confirmLabel: 'Create',
    });
    if (title === null) return;

    await flushPendingSave();

    try {
        const doc = await api('/documents', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ title, content: '' }),
        });
        state.owned.unshift(doc);
        renderDocumentLists();
        openDocument(doc.id);
        toast('Document created', 'success');
    } catch (error) {
        toast(`Could not create document: ${error.message}`, 'error');
    }
}

async function openDocument(id) {
    if (state.current && state.current.id === id && !state.dirty) return;
    await flushPendingSave();

    try {
        // Always read the stored version so the editor shows persisted content.
        const doc = await api(`/documents/${encodeURIComponent(id)}`);
        state.current = doc;
        state.dirty = false;

        el.docTitle.value = doc.title;
        el.editor.innerHTML = doc.content || '';
        el.docMeta.textContent = `Edited ${formatDate(doc.updated_at)}`;

        el.editorContainer.hidden = false;
        el.emptyState.hidden = true;
        el.sidebar.classList.remove('is-open');
        el.sidebarToggle.setAttribute('aria-expanded', 'false');

        applyAccessMode(doc);
        upsertLocalDocument(doc);
        renderDocumentLists();
        updateWordCount();
        setStatus('', null);
        if (doc.access !== 'view') el.editor.focus();
    } catch (error) {
        toast(`Could not open document: ${error.message}`, 'error');
    }
}

/// Reflects the caller's access level in the editor chrome.
function applyAccessMode(doc) {
    const canEdit = doc.access !== 'view';
    const isOwner = doc.access === 'owner';

    el.editor.setAttribute('contenteditable', String(canEdit));
    el.docTitle.readOnly = !canEdit;
    el.saveBtn.disabled = !canEdit;
    el.videoBtn.disabled = !canEdit;
    el.tools.forEach((tool) => { tool.disabled = !canEdit; });
    el.blockStyle.disabled = !canEdit;

    el.deleteBtn.hidden = !isOwner;
    el.shareBtn.hidden = !isOwner;

    el.accessBadge.hidden = isOwner;
    if (!isOwner) {
        el.accessBadge.textContent = canEdit
            ? `Shared by ${doc.owner_id} · can edit`
            : `Shared by ${doc.owner_id} · view only`;
        el.accessBadge.dataset.variant = canEdit ? 'edit' : 'view';
    }
}

async function saveDocument({ silent = true } = {}) {
    if (!state.current || state.saving) return;
    if (state.current.access === 'view') return;
    if (silent && !state.dirty) return;

    clearTimeout(autosaveTimer);
    autosaveTimer = null;
    state.saving = true;
    setStatus('Saving…', 'saving');

    const payload = {
        title: el.docTitle.value,
        content: el.editor.innerHTML,
        // Optimistic concurrency: the server rejects this write with 409 if the
        // stored document moved on since it was opened or last saved.
        revision: state.current.revision,
    };

    try {
        const doc = await api(`/documents/${encodeURIComponent(state.current.id)}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        });
        state.current = doc;
        state.dirty = false;
        el.docTitle.value = doc.title;
        // Content is re-rendered from the sanitized server copy so what is shown
        // is exactly what is stored.
        if (doc.content !== payload.content) el.editor.innerHTML = doc.content;
        el.docMeta.textContent = `Edited ${formatDate(doc.updated_at)}`;
        upsertLocalDocument(doc);
        renderDocumentLists();
        setStatus('All changes saved', 'saved');
    } catch (error) {
        if (error.status === 409 && error.body && error.body.document) {
            state.saving = false;
            await resolveConflict(error.body.document, payload);
            return;
        }
        setStatus('Unsaved changes', 'error');
        toast(`Could not save: ${error.message}`, 'error');
    } finally {
        state.saving = false;
    }
}

/// Someone else saved this document while it was open. Nothing is discarded
/// automatically: the local version is stashed first, then the user chooses to
/// overwrite the newer version or to load it.
async function resolveConflict(serverDoc, localPayload) {
    setStatus('Conflict - not saved', 'error');
    stashConflictCopy(serverDoc.id, localPayload);

    const answer = await promptDialog({
        title: 'Someone else edited this document',
        message: `${serverDoc.owner_id === currentUserId() ? 'Another session' : 'A collaborator'} `
            + `saved a newer version (revision ${serverDoc.revision}). `
            + 'Type OVERWRITE to replace it with your version, or cancel to load theirs. '
            + 'Your text is kept in this browser either way.',
        value: '',
        confirmLabel: 'Continue',
    });

    if (answer !== null && answer.trim().toUpperCase() === 'OVERWRITE') {
        state.current.revision = serverDoc.revision;
        state.dirty = true;
        await saveDocument({ silent: false });
        return;
    }

    state.current = serverDoc;
    state.dirty = false;
    el.docTitle.value = serverDoc.title;
    el.editor.innerHTML = serverDoc.content || '';
    el.docMeta.textContent = `Edited ${formatDate(serverDoc.updated_at)}`;
    applyAccessMode(serverDoc);
    upsertLocalDocument(serverDoc);
    renderDocumentLists();
    updateWordCount();
    setStatus('Loaded the newer version', 'saved');
    toast('Loaded the newer version. Your unsaved text was kept in this browser.', 'info');
}

/// Keeps the rejected local version in this browser so a conflict can never
/// destroy work outright.
function stashConflictCopy(id, payload) {
    try {
        window.localStorage.setItem(
            `docs-clone:conflict:${id}`,
            JSON.stringify({ saved_at: new Date().toISOString(), payload }),
        );
    } catch (_) { /* storage unavailable: nothing else to do */ }
}

async function deleteCurrentDocument() {
    if (!state.current) return;
    const confirmed = await promptDialog({
        title: 'Delete document',
        message: `“${state.current.title}” will be permanently deleted. Type DELETE to confirm.`,
        value: '',
        confirmLabel: 'Delete',
    });
    if (confirmed === null || confirmed.trim().toUpperCase() !== 'DELETE') return;

    const id = state.current.id;
    try {
        await api(`/documents/${encodeURIComponent(id)}`, { method: 'DELETE' });
        state.owned = state.owned.filter((doc) => doc.id !== id);
        state.dirty = false;
        closeEditor();
        renderDocumentLists();
        toast('Document deleted', 'success');
    } catch (error) {
        toast(`Could not delete document: ${error.message}`, 'error');
    }
}

async function handleFileUpload(event) {
    const file = event.target.files[0];
    event.target.value = '';
    if (!file) return;

    const extension = file.name.split('.').pop().toLowerCase();
    if (!['txt', 'md'].includes(extension)) {
        toast('Only .txt and .md files can be imported', 'error');
        return;
    }

    await flushPendingSave();

    const formData = new FormData();
    formData.append('file', file);

    setStatus('Importing…', 'saving');
    try {
        const doc = await api('/documents/upload', { method: 'POST', body: formData });
        state.owned.unshift(doc);
        renderDocumentLists();
        await openDocument(doc.id);
        toast(`Imported “${doc.title}”`, 'success');
    } catch (error) {
        setStatus('', null);
        toast(`Could not import file: ${error.message}`, 'error');
    }
}

/* ---------- Sharing ---------- */

async function openShareDialog() {
    if (!state.current || state.current.access !== 'owner') return;
    el.shareUserId.value = '';
    el.sharePermission.value = 'edit';
    el.shareBackdrop.hidden = false;
    el.shareUserId.focus();
    await renderShareList();
}

function closeShareDialog() {
    el.shareBackdrop.hidden = true;
    el.shareBtn.focus();
}

async function renderShareList() {
    el.shareList.innerHTML = '';
    let shares = [];
    try {
        shares = await api(`/documents/${encodeURIComponent(state.current.id)}/shares`);
    } catch (error) {
        toast(`Could not load sharing list: ${error.message}`, 'error');
        return;
    }

    if (shares.length === 0) {
        const empty = document.createElement('li');
        empty.className = 'list-empty';
        empty.textContent = 'Only you have access right now.';
        el.shareList.appendChild(empty);
        return;
    }

    shares.forEach((share) => el.shareList.appendChild(shareRow(share)));
}

function shareRow(share) {
    const row = document.createElement('li');
    row.className = 'share-row';

    const who = document.createElement('span');
    who.className = 'share-row__user';
    who.textContent = share.user_id;

    const perm = document.createElement('span');
    perm.className = 'share-row__perm';
    perm.textContent = share.permission === 'edit' ? 'Can edit' : 'Can view';

    const revoke = document.createElement('button');
    revoke.type = 'button';
    revoke.className = 'btn btn--danger btn--sm';
    revoke.textContent = 'Revoke';
    revoke.addEventListener('click', () => revokeAccess(share.user_id));

    row.append(who, perm, revoke);
    return row;
}

async function grantAccess(event) {
    event.preventDefault();
    if (!state.current) return;

    const userId = el.shareUserId.value.trim();
    if (!userId) return;

    try {
        await api(`/documents/${encodeURIComponent(state.current.id)}/shares`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ user_id: userId, permission: el.sharePermission.value }),
        });
        el.shareUserId.value = '';
        toast(`Shared with ${userId}`, 'success');
        await renderShareList();
        await refreshCurrentMeta();
    } catch (error) {
        toast(`Could not share: ${error.message}`, 'error');
    }
}

async function revokeAccess(userId) {
    try {
        await api(
            `/documents/${encodeURIComponent(state.current.id)}/shares/${encodeURIComponent(userId)}`,
            { method: 'DELETE' },
        );
        toast(`Access revoked for ${userId}`, 'success');
        await renderShareList();
        await refreshCurrentMeta();
    } catch (error) {
        toast(`Could not revoke access: ${error.message}`, 'error');
    }
}

async function refreshCurrentMeta() {
    if (!state.current) return;
    try {
        const doc = await api(`/documents/${encodeURIComponent(state.current.id)}`);
        state.current = Object.assign({}, state.current, {
            shared_with_count: doc.shared_with_count,
        });
        upsertLocalDocument(state.current);
        renderDocumentLists();
    } catch (_) { /* non-critical */ }
}

/* ---------- YouTube embed ---------- */

function openVideoDialog() {
    if (!state.current || state.current.access === 'view') return;
    savedRange = currentEditorRange();
    el.videoUrl.value = '';
    el.videoError.hidden = true;
    el.videoBackdrop.hidden = false;
    el.videoUrl.focus();
}

function closeVideoDialog() {
    el.videoBackdrop.hidden = true;
    el.editor.focus();
}

/// Extracts a YouTube video id from a URL or a bare id. Returns null otherwise.
function parseYouTubeId(raw) {
    const value = raw.trim();
    if (!value) return null;

    if (/^[A-Za-z0-9_-]{11}$/.test(value)) return value;

    let url;
    try {
        url = new URL(value);
    } catch (_) {
        return null;
    }

    if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;

    const host = url.hostname.toLowerCase().replace(/^m\./, '');
    let id = null;

    if (host === 'youtu.be') {
        id = url.pathname.slice(1);
    } else if (['youtube.com', 'www.youtube.com', 'youtube-nocookie.com', 'www.youtube-nocookie.com'].includes(host)) {
        if (url.pathname === '/watch') id = url.searchParams.get('v');
        else if (url.pathname.startsWith('/embed/')) id = url.pathname.slice('/embed/'.length);
        else if (url.pathname.startsWith('/shorts/')) id = url.pathname.slice('/shorts/'.length);
    }

    if (!id) return null;
    id = id.split('/')[0];
    return /^[A-Za-z0-9_-]{11}$/.test(id) ? id : null;
}

function insertVideo() {
    const videoId = parseYouTubeId(el.videoUrl.value);
    if (!videoId) {
        el.videoError.textContent = 'Enter a valid YouTube link or 11-character video id.';
        el.videoError.hidden = false;
        return;
    }

    const iframe = document.createElement('iframe');
    iframe.src = `https://www.youtube-nocookie.com/embed/${videoId}`;
    iframe.width = '560';
    iframe.height = '315';
    iframe.title = 'YouTube video';
    iframe.loading = 'lazy';
    iframe.setAttribute('allowfullscreen', '');
    iframe.setAttribute('referrerpolicy', 'strict-origin-when-cross-origin');

    const wrapper = document.createElement('div');
    wrapper.className = 'embed';
    wrapper.appendChild(iframe);

    const spacer = document.createElement('p');
    spacer.appendChild(document.createElement('br'));

    el.editor.focus();
    const range = savedRange && el.editor.contains(savedRange.commonAncestorContainer)
        ? savedRange
        : null;

    if (range) {
        range.deleteContents();
        range.insertNode(spacer);
        range.insertNode(wrapper);
    } else {
        el.editor.appendChild(wrapper);
        el.editor.appendChild(spacer);
    }

    closeVideoDialog();
    markDirty();
    updateWordCount();
}

function currentEditorRange() {
    const selection = document.getSelection();
    if (!selection || selection.rangeCount === 0) return null;
    const range = selection.getRangeAt(0);
    return el.editor.contains(range.commonAncestorContainer) ? range.cloneRange() : null;
}

/* ---------- Rendering ---------- */

function renderDocumentLists() {
    el.ownedCount.textContent = String(state.owned.length);
    el.sharedCount.textContent = String(state.shared.length);

    renderList(el.ownedList, state.owned, 'No documents yet. Create or import one.');
    renderList(el.sharedList, state.shared, 'Nothing shared with you yet.');
}

function renderList(container, docs, emptyMessage) {
    const visible = docs.filter((doc) =>
        !state.filter || doc.title.toLowerCase().includes(state.filter));

    container.innerHTML = '';

    if (visible.length === 0) {
        const empty = document.createElement('p');
        empty.className = 'list-empty';
        empty.textContent = docs.length === 0 ? emptyMessage : 'No documents match your search.';
        container.appendChild(empty);
        return;
    }

    visible.forEach((doc) => container.appendChild(docCard(doc)));
}

function docCard(doc) {
    const card = document.createElement('button');
    card.type = 'button';
    card.className = 'doc-card';
    card.setAttribute('role', 'listitem');
    if (state.current && state.current.id === doc.id) {
        card.setAttribute('aria-current', 'true');
    }

    const title = document.createElement('span');
    title.className = 'doc-card__title';
    title.textContent = doc.title || 'Untitled document';

    const meta = document.createElement('span');
    meta.className = 'doc-card__meta';
    if (doc.access === 'owner') {
        const shared = doc.shared_with_count > 0
            ? ` · shared with ${doc.shared_with_count}`
            : '';
        meta.textContent = `Edited ${formatDate(doc.updated_at)}${shared}`;
    } else {
        meta.textContent = `${doc.owner_id} · ${doc.access === 'edit' ? 'can edit' : 'view only'}`;
    }

    card.append(title, meta);

    if (doc.access !== 'owner') {
        const tag = document.createElement('span');
        tag.className = 'doc-card__tag';
        tag.textContent = 'Shared';
        card.appendChild(tag);
    }

    card.addEventListener('click', () => openDocument(doc.id));
    return card;
}

function closeEditor() {
    state.current = null;
    state.dirty = false;
    clearTimeout(autosaveTimer);
    autosaveTimer = null;
    el.editor.innerHTML = '';
    el.docTitle.value = '';
    el.editorContainer.hidden = true;
    el.emptyState.hidden = false;
    el.shareBackdrop.hidden = true;
    el.videoBackdrop.hidden = true;
    setStatus('', null);
    renderDocumentLists();
}

function markDirty() {
    if (!state.current || state.current.access === 'view') return;
    state.dirty = true;
    setStatus('Unsaved changes', null);
    clearTimeout(autosaveTimer);
    autosaveTimer = setTimeout(() => saveDocument({ silent: true }), AUTOSAVE_DELAY);
}

async function flushPendingSave() {
    if (state.dirty && state.current) {
        await saveDocument({ silent: true });
    }
}

function setStatus(text, stateName) {
    el.saveStatus.textContent = text;
    if (stateName) {
        el.saveStatus.dataset.state = stateName;
    } else {
        delete el.saveStatus.dataset.state;
    }
}

function updateWordCount() {
    const text = el.editor.innerText.trim();
    const words = text ? text.split(/\s+/).length : 0;
    el.wordCount.textContent = `${words} word${words === 1 ? '' : 's'}`;
}

function runCommand(command, value) {
    if (!command) return;
    if (!state.current || state.current.access === 'view') return;
    el.editor.focus();
    document.execCommand(command, false, value);
    markDirty();
    updateWordCount();
    syncToolbarState();
}

function syncToolbarState() {
    if (el.editorContainer.hidden) return;
    const anchor = document.getSelection() ? document.getSelection().anchorNode : null;
    if (!anchor || !el.editor.contains(anchor)) return;

    el.tools.forEach((tool) => {
        const command = tool.dataset.command;
        if (['bold', 'italic', 'underline', 'insertUnorderedList', 'insertOrderedList'].includes(command)) {
            let active = false;
            try { active = document.queryCommandState(command); } catch (_) { active = false; }
            tool.setAttribute('aria-pressed', String(active));
        }
    });

    let block = 'p';
    try {
        block = (document.queryCommandValue('formatBlock') || 'p').toLowerCase();
    } catch (_) { /* ignore */ }
    el.blockStyle.value = ['h1', 'h2', 'h3'].includes(block) ? block : 'p';
}

/* ---------- Helpers ---------- */

function upsertLocalDocument(doc) {
    const list = doc.access === 'owner' ? state.owned : state.shared;
    const index = list.findIndex((item) => item.id === doc.id);
    if (index === -1) {
        list.unshift(doc);
    } else {
        list[index] = Object.assign({}, list[index], doc);
    }
    list.sort((a, b) => new Date(b.updated_at) - new Date(a.updated_at));
}

function formatDate(value) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return 'just now';
    const sameDay = date.toDateString() === new Date().toDateString();
    return sameDay
        ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
        : date.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
}

function toast(message, variant = 'info') {
    const node = document.createElement('div');
    node.className = `toast toast--${variant}`;
    node.textContent = message;
    el.toasts.appendChild(node);
    setTimeout(() => node.remove(), 4000);
}

let dialogResolver = null;

function promptDialog({ title, message, value = '', confirmLabel = 'Confirm' }) {
    el.dialogTitle.textContent = title;
    el.dialogMessage.textContent = message;
    el.dialogInput.value = value;
    el.dialogConfirm.textContent = confirmLabel;
    el.dialogBackdrop.hidden = false;
    el.dialogInput.focus();
    el.dialogInput.select();

    return new Promise((resolve) => {
        dialogResolver = resolve;
    });
}

function closeDialog(result) {
    el.dialogBackdrop.hidden = true;
    if (dialogResolver) {
        dialogResolver(result);
        dialogResolver = null;
    }
}

el.dialogConfirm.addEventListener('click', () => closeDialog(el.dialogInput.value));
el.dialogCancel.addEventListener('click', () => closeDialog(null));
el.dialogInput.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') closeDialog(el.dialogInput.value);
});
el.dialogBackdrop.addEventListener('click', (event) => {
    if (event.target === el.dialogBackdrop) closeDialog(null);
});
