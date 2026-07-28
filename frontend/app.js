const API_BASE = '/api';
const AUTOSAVE_DELAY = 1200;

const state = {
    documents: [],
    current: null,
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
    documentList: document.getElementById('documentList'),
    docCount: document.getElementById('docCount'),
    editorContainer: document.getElementById('editorContainer'),
    emptyState: document.getElementById('emptyState'),
    docTitle: document.getElementById('docTitle'),
    docMeta: document.getElementById('docMeta'),
    editor: document.getElementById('editor'),
    saveBtn: document.getElementById('saveBtn'),
    deleteBtn: document.getElementById('deleteBtn'),
    closeBtn: document.getElementById('closeBtn'),
    saveStatus: document.getElementById('saveStatus'),
    wordCount: document.getElementById('wordCount'),
    blockStyle: document.getElementById('blockStyle'),
    tools: Array.from(document.querySelectorAll('.tool')),
    sidebar: document.getElementById('sidebar'),
    sidebarToggle: document.getElementById('sidebarToggle'),
    toasts: document.getElementById('toasts'),
    dialogBackdrop: document.getElementById('dialogBackdrop'),
    dialogTitle: document.getElementById('dialogTitle'),
    dialogMessage: document.getElementById('dialogMessage'),
    dialogInput: document.getElementById('dialogInput'),
    dialogConfirm: document.getElementById('dialogConfirm'),
    dialogCancel: document.getElementById('dialogCancel'),
};

let autosaveTimer = null;

document.addEventListener('DOMContentLoaded', () => {
    setupEventListeners();
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

    el.userId.addEventListener('change', async () => {
        await flushPendingSave();
        closeEditor();
        loadDocuments();
    });

    el.searchInput.addEventListener('input', () => {
        state.filter = el.searchInput.value.trim().toLowerCase();
        renderDocumentList();
    });

    el.editor.addEventListener('input', () => {
        markDirty();
        updateWordCount();
    });
    el.docTitle.addEventListener('input', markDirty);

    el.editor.addEventListener('keyup', syncToolbarState);
    el.editor.addEventListener('mouseup', syncToolbarState);
    document.addEventListener('selectionchange', syncToolbarState);

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
        if (event.key === 'Escape' && !el.dialogBackdrop.hidden) {
            closeDialog(null);
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

async function api(path, options = {}) {
    const response = await fetch(`${API_BASE}${path}`, options);
    if (!response.ok) {
        let message = `Request failed (${response.status})`;
        try {
            const body = await response.json();
            if (body && body.error) message = body.error;
        } catch (_) { /* keep default message */ }
        throw new Error(message);
    }
    return response.status === 204 ? null : response.json();
}

async function loadDocuments() {
    const userId = currentUserId();
    el.documentList.setAttribute('aria-busy', 'true');
    try {
        state.documents = await api(`/documents?owner_id=${encodeURIComponent(userId)}`);
    } catch (error) {
        state.documents = [];
        toast(`Could not load documents: ${error.message}`, 'error');
    } finally {
        el.documentList.setAttribute('aria-busy', 'false');
        renderDocumentList();
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
            body: JSON.stringify({
                title: title.trim() || 'Untitled document',
                content: '',
                owner_id: currentUserId(),
            }),
        });
        state.documents.unshift(doc);
        renderDocumentList();
        openDocument(doc.id);
        toast('Document created', 'success');
    } catch (error) {
        toast(`Could not create document: ${error.message}`, 'error');
    }
}

async function openDocument(id) {
    if (state.current && state.current.id === id && !state.dirty) {
        return;
    }
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

        upsertLocalDocument(doc);
        renderDocumentList();
        updateWordCount();
        setStatus('', null);
        el.editor.focus();
    } catch (error) {
        toast(`Could not open document: ${error.message}`, 'error');
    }
}

async function saveDocument({ silent = true } = {}) {
    if (!state.current || state.saving) return;
    if (silent && !state.dirty) return;

    clearTimeout(autosaveTimer);
    autosaveTimer = null;
    state.saving = true;
    setStatus('Saving…', 'saving');

    const payload = {
        title: el.docTitle.value.trim() || 'Untitled document',
        content: el.editor.innerHTML,
    };

    try {
        const doc = await api(`/documents/${encodeURIComponent(state.current.id)}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        });
        state.current = doc;
        state.dirty = false;
        el.docMeta.textContent = `Edited ${formatDate(doc.updated_at)}`;
        upsertLocalDocument(doc);
        renderDocumentList();
        setStatus('All changes saved', 'saved');
    } catch (error) {
        setStatus('Unsaved changes', 'error');
        toast(`Could not save: ${error.message}`, 'error');
    } finally {
        state.saving = false;
    }
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
        state.documents = state.documents.filter((doc) => doc.id !== id);
        state.dirty = false;
        closeEditor();
        renderDocumentList();
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
        const doc = await api(`/documents/upload?owner_id=${encodeURIComponent(currentUserId())}`, {
            method: 'POST',
            body: formData,
        });
        state.documents.unshift(doc);
        renderDocumentList();
        await openDocument(doc.id);
        toast(`Imported “${doc.title}”`, 'success');
    } catch (error) {
        setStatus('', null);
        toast(`Could not import file: ${error.message}`, 'error');
    }
}

/* ---------- Rendering ---------- */

function renderDocumentList() {
    const visible = state.documents.filter((doc) =>
        !state.filter || doc.title.toLowerCase().includes(state.filter));

    el.docCount.textContent = String(state.documents.length);
    el.documentList.innerHTML = '';

    if (visible.length === 0) {
        const empty = document.createElement('p');
        empty.className = 'list-empty';
        empty.textContent = state.documents.length === 0
            ? 'No documents yet. Create or import one.'
            : 'No documents match your search.';
        el.documentList.appendChild(empty);
        return;
    }

    visible.forEach((doc) => {
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
        meta.textContent = `Edited ${formatDate(doc.updated_at)}`;

        card.append(title, meta);
        card.addEventListener('click', () => openDocument(doc.id));
        el.documentList.appendChild(card);
    });
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
    setStatus('', null);
    renderDocumentList();
}

function markDirty() {
    if (!state.current) return;
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
    el.editor.focus();
    document.execCommand(command, false, value);
    markDirty();
    updateWordCount();
    syncToolbarState();
}

function syncToolbarState() {
    if (el.editorContainer.hidden) return;
    if (!el.editor.contains(document.getSelection()?.anchorNode || null)) return;

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

function currentUserId() {
    return el.userId.value.trim() || 'user1';
}

function upsertLocalDocument(doc) {
    const index = state.documents.findIndex((item) => item.id === doc.id);
    if (index === -1) {
        state.documents.unshift(doc);
    } else {
        state.documents[index] = doc;
    }
    state.documents.sort((a, b) => new Date(b.updated_at) - new Date(a.updated_at));
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
