const API_BASE = '/api';
let currentDocument = null;
let documents = [];

// DOM Elements
const userIdInput = document.getElementById('userId');
const newDocBtn = document.getElementById('newDocBtn');
const uploadBtn = document.getElementById('uploadBtn');
const fileUpload = document.getElementById('fileUpload');
const documentList = document.getElementById('documentList');
const editorContainer = document.getElementById('editorContainer');
const emptyState = document.getElementById('emptyState');
const docTitle = document.getElementById('docTitle');
const editor = document.getElementById('editor');
const saveBtn = document.getElementById('saveBtn');
const closeBtn = document.getElementById('closeBtn');
const toolbarBtns = document.querySelectorAll('.toolbar-btn');

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    loadDocuments();
    setupEventListeners();
});

function setupEventListeners() {
    newDocBtn.addEventListener('click', createNewDocument);
    uploadBtn.addEventListener('click', () => fileUpload.click());
    fileUpload.addEventListener('change', handleFileUpload);
    saveBtn.addEventListener('click', saveDocument);
    closeBtn.addEventListener('click', closeEditor);
    
    toolbarBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            const command = btn.dataset.command;
            const value = btn.dataset.value || null;
            document.execCommand(command, false, value);
            editor.focus();
        });
    });

    // Auto-save on input (debounced)
    let saveTimeout;
    editor.addEventListener('input', () => {
        clearTimeout(saveTimeout);
        saveTimeout = setTimeout(() => {
            if (currentDocument) {
                saveDocument();
            }
        }, 2000);
    });

    docTitle.addEventListener('input', () => {
        clearTimeout(saveTimeout);
        saveTimeout = setTimeout(() => {
            if (currentDocument) {
                saveDocument();
            }
        }, 2000);
    });
}

async function loadDocuments() {
    const userId = userIdInput.value;
    try {
        const response = await fetch(`${API_BASE}/documents?owner_id=${userId}`);
        if (response.ok) {
            documents = await response.json();
            renderDocumentList();
        }
    } catch (error) {
        console.error('Error loading documents:', error);
    }
}

function renderDocumentList() {
    documentList.innerHTML = '';
    documents.forEach(doc => {
        const item = document.createElement('div');
        item.className = 'document-item';
        if (currentDocument && currentDocument.id === doc.id) {
            item.classList.add('active');
        }
        
        const date = new Date(doc.updated_at).toLocaleDateString();
        item.innerHTML = `
            <h3>${escapeHtml(doc.title)}</h3>
            <p>Last edited: ${date}</p>
        `;
        
        item.addEventListener('click', () => openDocument(doc));
        documentList.appendChild(item);
    });
}

async function createNewDocument() {
    const userId = userIdInput.value;
    const title = prompt('Enter document title:', 'Untitled Document');
    
    if (!title) return;
    
    try {
        const response = await fetch(`${API_BASE}/documents`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                title: title,
                content: '',
                owner_id: userId
            })
        });
        
        if (response.ok) {
            const newDoc = await response.json();
            documents.unshift(newDoc);
            renderDocumentList();
            openDocument(newDoc);
        }
    } catch (error) {
        console.error('Error creating document:', error);
        alert('Failed to create document');
    }
}

async function openDocument(doc) {
    currentDocument = doc;
    docTitle.value = doc.title;
    editor.innerHTML = doc.content;
    
    editorContainer.style.display = 'flex';
    emptyState.style.display = 'none';
    
    renderDocumentList();
}

async function saveDocument() {
    if (!currentDocument) return;
    
    const content = editor.innerHTML;
    console.log('Saving document:', currentDocument.id, 'content:', content);
    
    try {
        const response = await fetch(`${API_BASE}/documents/${currentDocument.id}`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                title: docTitle.value,
                content: content
            })
        });
        
        if (response.ok) {
            const updatedDoc = await response.json();
            currentDocument = updatedDoc;
            
            // Update documents array
            const index = documents.findIndex(d => d.id === updatedDoc.id);
            if (index !== -1) {
                documents[index] = updatedDoc;
            }
            
            renderDocumentList();
            saveBtn.textContent = 'Saved!';
            setTimeout(() => {
                saveBtn.textContent = 'Save';
            }, 2000);
        }
    } catch (error) {
        console.error('Error saving document:', error);
        alert('Failed to save document');
    }
}

function closeEditor() {
    currentDocument = null;
    editorContainer.style.display = 'none';
    emptyState.style.display = 'flex';
    renderDocumentList();
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Reload documents when user ID changes
userIdInput.addEventListener('change', () => {
    loadDocuments();
    closeEditor();
});

async function handleFileUpload(event) {
    const file = event.target.files[0];
    if (!file) return;
    
    const userId = userIdInput.value;
    const formData = new FormData();
    formData.append('file', file);
    
    try {
        const response = await fetch(`${API_BASE}/documents/upload?owner_id=${userId}`, {
            method: 'POST',
            body: formData
        });
        
        if (response.ok) {
            const newDoc = await response.json();
            documents.unshift(newDoc);
            renderDocumentList();
            openDocument(newDoc);
        } else {
            alert('Failed to upload file');
        }
    } catch (error) {
        console.error('Error uploading file:', error);
        alert('Failed to upload file');
    }
    
    // Reset file input
    event.target.value = '';
}
