import { initLang, setLangPref, getLangPref, applyStaticI18n } from './i18n.js';

const tauriCore = window.__TAURI__?.core;
const invoke = tauriCore?.invoke?.bind(tauriCore) ?? (async (command) => {
    throw new Error(`浏览器预览模式不支持原生命令：${command}`);
});

const DEFAULT_ANALYZE_CONCURRENCY = 4;

const state = {
    results: [],
    selectedPath: null,
    detailOverride: null,
    detailTab: 'basic',
    currentPath: '~/Library/Caches',
    browserPanel: 'home',
    hasScanned: false,
    isScanning: false,
    scanError: null,
    filter: 'all',
    sort: { key: 'name', direction: 'asc' },
    showSizeColumn: localStorage.getItem('showSizeColumn') !== 'false',
    analyzeConcurrency: Number(localStorage.getItem('analyzeConcurrency')) || DEFAULT_ANALYZE_CONCURRENCY,
    customLocations: [],
    resultCache: new Map(),
    config: {
        provider: 'zhipu', base_url: 'https://open.bigmodel.cn/api/paas/v4',
        model: 'glm-5.2', has_api_key: false, config_path: '',
    },
};

const providerDefaults = {
    zhipu: { baseUrl: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-5.2' },
    openai: { baseUrl: 'https://api.openai.com/v1', model: '' },
    deepseek: { baseUrl: 'https://api.deepseek.com', model: 'deepseek-chat' },
    openrouter: { baseUrl: 'https://openrouter.ai/api/v1', model: '' },
    custom: { baseUrl: '', model: '' },
};

const NAV_ICONS = {
    overview: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"><rect x="2" y="2" width="5" height="5" rx="1"/><rect x="9" y="2" width="5" height="5" rx="1"/><rect x="2" y="9" width="5" height="5" rx="1"/><rect x="9" y="9" width="5" height="5" rx="1"/></svg>',
    cache: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><rect x="2" y="3" width="12" height="3" rx="0.6"/><path d="M3 6v6.5a.5.5 0 0 0 .5.5h9a.5.5 0 0 0 .5-.5V6"/><path d="M6.5 9h3"/></svg>',
    appsupport: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M2 6h12"/></svg>',
    home: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" stroke-linecap="round"><path d="M2.6 7.2 8 3l5.4 4.2"/><path d="M4.3 6.4V13h7.4V6.4"/></svg>',
    add: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M8 3.5v9M3.5 8h9"/></svg>',
    settings: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><path d="M2.5 5h11M2.5 11h11"/><circle cx="6" cy="5" r="1.9" fill="currentColor" stroke="none"/><circle cx="10.5" cy="11" r="1.9" fill="currentColor" stroke="none"/></svg>',
    folder: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M2.5 5.4a1 1 0 0 1 1-1h2.3l1.1 1.4h5.6a1 1 0 0 1 1 1v5.2a1 1 0 0 1-1 1h-9.6a1 1 0 0 1-1-1z"/></svg>',
};

const elements = {};
let rowClickTimer = null;
let scanRequestId = 0;
let isBatchAnalyzing = false;
let analysisAbort = false;

function formatSize(bytes) {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let value = Number(bytes) || 0;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit += 1;
    }
    const digits = unit === 0 ? 0 : 1;
    return `${value.toFixed(digits)} ${units[unit]}`;
}

function formatDate(value) {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? '未知' : date.toLocaleString('zh-CN', { dateStyle: 'medium' });
}

function formatDuration(ms) {
    const totalSeconds = Math.max(0, Math.round(ms / 1000));
    if (totalSeconds < 60) return `${totalSeconds} 秒`;
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes} 分 ${String(seconds).padStart(2, '0')} 秒`;
}

function usageKind(result) {
    if (result.uiState === 'queued') return 'queued';
    if (result.uiState === 'analyzing') return 'analyzing';
    if (result.uiState === 'failed') return 'error';
    if (result.verdict.is_residue) return 'residue';
    if (result.verdict.deletable === 'never') return 'active';
    return 'unknown';
}

function usageLabel(result) {
    const labels = {
        queued: '等待分析', analyzing: '分析中', error: '分析失败', residue: '疑似残留',
        active: '正在使用', unknown: '无法确认',
    };
    return labels[usageKind(result)] || '无法确认';
}

function usageSymbol(result) {
    return { queued: '◌', analyzing: '◌', error: '!', residue: '◆', active: '●', unknown: '○' }[usageKind(result)] || '○';
}

function impactKind(verdict) {
    return {
        safe: 'regenerable', caution: 'configuration', never: 'protected', unknown: 'unconfirmed',
    }[verdict.deletable] || 'unconfirmed';
}

function impactLabel(verdict) {
    return {
        regenerable: '可重新生成', configuration: '可能影响配置',
        protected: '受保护', unconfirmed: '影响未确认',
    }[impactKind(verdict)];
}

function adviceShortLabel(verdict) {
    return {
        regenerable: '可删', configuration: '谨慎', protected: '保留', unconfirmed: '未分析',
    }[impactKind(verdict)];
}

function sourceLabel(source) {
    return { local_rule: '内置规则', ai: 'AI 分析', cache: '本地缓存', unknown: '未分析' }[source] || '未分析';
}

function priority(result) {
    const impactPriority = { regenerable: 0, configuration: 1, unconfirmed: 2, protected: 3 }[impactKind(result.verdict)] ?? 4;
    return usageKind(result) === 'residue' ? -1 : impactPriority;
}

function visibleResults() {
    const direction = state.sort.direction === 'asc' ? 1 : -1;
    return state.results
        .filter((result) => {
            if (state.filter === 'actionable') return ['regenerable', 'configuration'].includes(impactKind(result.verdict));
            if (state.filter === 'unknown') return ['unknown', 'error'].includes(usageKind(result));
            if (state.filter === 'hide_whitelist') return !result.is_whitelisted;
            if (state.filter === 'whitelist') return result.is_whitelisted;
            if (state.filter === 'hide_hidden') return !result.directory.name.startsWith('.');
            if (state.filter === 'hidden') return result.directory.name.startsWith('.');
            return true;
        })
        .sort((a, b) => {
            let comparison = 0;
            if (state.sort.key === 'name') {
                comparison = a.directory.name.localeCompare(b.directory.name, 'zh-CN', { sensitivity: 'base' });
            } else if (state.sort.key === 'purpose') {
                comparison = (a.verdict.purpose || '').localeCompare(b.verdict.purpose || '', 'zh-CN');
            } else if (state.sort.key === 'status') {
                comparison = usageLabel(a).localeCompare(usageLabel(b), 'zh-CN');
            } else if (state.sort.key === 'size') {
                const aKnown = a.sizeState === 'precise';
                const bKnown = b.sizeState === 'precise';
                if (aKnown !== bKnown) return aKnown ? -1 : 1;
                comparison = a.directory.size - b.directory.size;
            } else if (state.sort.key === 'modified') {
                comparison = new Date(a.directory.last_modified).getTime() - new Date(b.directory.last_modified).getTime();
            }
            return comparison * direction || a.directory.name.localeCompare(b.directory.name, 'zh-CN');
        });
}

function changeSort(key) {
    if (state.sort.key === key) {
        state.sort.direction = state.sort.direction === 'asc' ? 'desc' : 'asc';
    } else {
        state.sort = { key, direction: key === 'modified' || key === 'size' ? 'desc' : 'asc' };
    }
    renderSortHeaders();
    renderList();
}

function renderSortHeaders() {
    // 表头文字由 data-i18n 负责，这里只切换排序状态类（升/降箭头是 CSS ::after）。
    document.querySelectorAll('[data-sort-key]').forEach((button) => {
        button.classList.toggle('active', state.sort.key === button.dataset.sortKey);
        button.classList.toggle('sort-asc', state.sort.key === button.dataset.sortKey && state.sort.direction === 'asc');
        button.classList.toggle('sort-desc', state.sort.key === button.dataset.sortKey && state.sort.direction === 'desc');
    });
}

function cacheResult(result) {
    state.resultCache.set(result.directory.path, result);
}

function createElement(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
}

function escapeHtml(text) {
    return text.replace(/[&<>"']/g, (char) => (
        { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]
    ));
}

// 极简 markdown → HTML。先转义再套标签，避免注入。够用于模型返回的说明性文本。
function renderInlineMarkdown(escaped) {
    return escaped
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
        .replace(/(^|[^*])\*([^*\n]+)\*(?!\*)/g, '$1<em>$2</em>');
}

function markdownToHtml(markdown) {
    const lines = escapeHtml((markdown || '').trim()).split('\n');
    let html = '';
    let list = null; // 'ul' | 'ol'
    let para = [];
    const flushPara = () => {
        if (para.length) { html += `<p>${renderInlineMarkdown(para.join(' '))}</p>`; para = []; }
    };
    const closeList = () => { if (list) { html += `</${list}>`; list = null; } };
    lines.forEach((raw) => {
        const line = raw.trim();
        if (!line) { flushPara(); closeList(); return; }
        const heading = line.match(/^#{1,6}\s+(.*)$/);
        const bullet = line.match(/^[-*+]\s+(.*)$/);
        const ordered = line.match(/^\d+\.\s+(.*)$/);
        if (heading) { flushPara(); closeList(); html += `<p class="md-heading">${renderInlineMarkdown(heading[1])}</p>`; }
        else if (bullet) { flushPara(); if (list !== 'ul') { closeList(); html += '<ul>'; list = 'ul'; } html += `<li>${renderInlineMarkdown(bullet[1])}</li>`; }
        else if (ordered) { flushPara(); if (list !== 'ol') { closeList(); html += '<ol>'; list = 'ol'; } html += `<li>${renderInlineMarkdown(ordered[1])}</li>`; }
        else { closeList(); para.push(line); }
    });
    flushPara();
    closeList();
    return html;
}

function createDebugBlock(label, content) {
    const block = createElement('section', 'debug-block');
    const header = createElement('div', 'debug-block-header');
    const copyButton = createElement('button', 'debug-copy-button', '复制');
    copyButton.type = 'button';
    copyButton.addEventListener('click', async () => {
        await navigator.clipboard.writeText(content);
        elements.scanStatus.textContent = `${label}已复制`;
    });
    header.append(createElement('h3', '', label), copyButton);
    block.append(header, createElement('pre', '', content));
    return block;
}

// 即席「AI 详解」：点按钮直接问模型这个路径是什么，展示原文，不保存。
// 问答状态挂在 result.aiExplain（仅内存），以便切 Tab / 重渲染时不丢失。
function createAiExplainSection(result) {
    const directory = result.directory;
    const section = createElement('section', 'ai-explain');
    const hint = createElement('p', 'field-note', '直接询问 AI 这个路径是什么，展示模型原文。结果不会保存 —— 省得你再去别处问一遍。');
    const body = createElement('div', 'ai-explain-body');

    const initial = result.aiExplain || { status: 'idle' };
    const askButton = createElement('button', 'primary-button', {
        idle: '问问 AI 这是什么', loading: '正在询问…', done: '重新询问', error: '重试',
    }[initial.status]);
    askButton.type = 'button';
    askButton.disabled = initial.status === 'loading';

    const renderBody = () => {
        const s = result.aiExplain || { status: 'idle' };
        if (s.status === 'loading') {
            const loading = createElement('div', 'ai-explain-loading');
            loading.append(createElement('span', 'scan-spinner'), createElement('span', '', '正在询问 AI…'));
            body.replaceChildren(loading);
        } else if (s.status === 'done') {
            const article = createElement('div', 'ai-explain-text markdown-body');
            if (s.text) article.innerHTML = markdownToHtml(s.text);
            else article.textContent = '（模型没有返回内容）';
            body.replaceChildren(article);
        } else if (s.status === 'error') {
            body.replaceChildren(createElement('p', 'ai-explain-error', `询问失败：${s.error}`));
        } else {
            body.replaceChildren(createElement('p', 'ai-explain-placeholder', '尚未询问。点上方按钮，让 AI 解释这个路径是干什么的。'));
        }
    };

    askButton.addEventListener('click', async () => {
        result.aiExplain = { status: 'loading' };
        askButton.textContent = '正在询问…';
        askButton.disabled = true;
        renderBody();
        try {
            const answer = await invoke('explain_path', { path: directory.path });
            result.aiExplain = { status: 'done', text: (answer || '').trim() };
            askButton.textContent = '重新询问';
        } catch (error) {
            result.aiExplain = { status: 'error', error: String(error) };
            askButton.textContent = '重试';
        } finally {
            askButton.disabled = false;
            renderBody();
        }
    });

    const header = createElement('div', 'ai-explain-header');
    header.append(hint, askButton);
    section.append(header, body);
    renderBody();
    return section;
}

function renderList() {
    const results = visibleResults();
    elements.directoryList.replaceChildren();
    elements.resultCount.textContent = state.isScanning ? '扫描中…' : `${results.length} 项`;

    if (state.isScanning) {
        const loading = createElement('div', 'scan-loading');
        const heading = createElement('div', 'scan-loading-heading');
        heading.append(
            createElement('span', 'scan-spinner'),
            createElement('div', '', `正在读取 ${state.currentPath}`),
        );
        loading.append(heading);
        for (let index = 0; index < 7; index += 1) {
            const row = createElement('div', 'skeleton-row');
            row.append(
                createElement('span', 'skeleton-block skeleton-name'),
                createElement('span', 'skeleton-block skeleton-purpose'),
                createElement('span', 'skeleton-block skeleton-advice'),
                createElement('span', 'skeleton-block skeleton-size'),
                createElement('span', 'skeleton-block skeleton-actions'),
            );
            loading.append(row);
        }
        elements.directoryList.append(loading);
        return;
    }

    if (state.scanError) {
        const error = createElement('div', 'scan-error');
        error.append(
            createElement('strong', '', '无法读取这个位置'),
            createElement('p', '', state.scanError),
        );
        const retry = createElement('button', 'primary-button', '重新扫描');
        retry.type = 'button';
        retry.addEventListener('click', () => scanDirectories(state.currentPath));
        error.append(retry);
        elements.directoryList.append(error);
        return;
    }

    if (results.length === 0) {
        if (!state.hasScanned) {
            const empty = createElement('div', 'home-empty');
            empty.append(
                (() => {
                    const logo = createElement('img', 'home-mark');
                    logo.src = '/dirdetective-logo.svg';
                    logo.alt = 'DirDetective';
                    return logo;
                })(),
                createElement('h2', '', '选择一个目录开始分析'),
                createElement('p', '', '看懂每个目录是干什么的，它是谁的、存了什么、删了会怎样，再由你自己决定要不要清理。'),
            );
            const actions = createElement('div', 'home-actions');
            const recommended = createElement('button', 'primary-button', '扫描缓存目录');
            recommended.type = 'button';
            recommended.addEventListener('click', () => openDirectory('~/Library/Caches'));
            const choose = createElement('button', '', '选择其他目录');
            choose.type = 'button';
            choose.addEventListener('click', chooseDirectory);
            actions.append(recommended, choose);
            empty.append(actions, createElement('span', 'privacy-note', '仅分析目录名与文件名，不读取文件内容'));
            elements.directoryList.append(empty);
            return;
        }
        const empty = createElement('div', 'list-empty', state.results.length ? '没有符合条件的项目' : '该位置没有可显示的项目');
        elements.directoryList.append(empty);
        return;
    }

    results.forEach((result) => {
        const row = createElement('div', 'file-row');
        row.classList.toggle('whitelisted', result.is_whitelisted);
        row.classList.toggle('selected', result.directory.path === state.selectedPath);
        row.classList.toggle('unanalyzed', result.verdict.source === 'unknown');
        row.classList.toggle('cache-stale', Boolean(result.cache_stale));
        row.classList.toggle('row-analyzing', result.uiState === 'analyzing');
        row.classList.toggle('row-queued', result.uiState === 'queued');
        row.tabIndex = 0;
        row.setAttribute('role', 'button');
        row.title = '单击查看详情，双击进入目录';
        row.addEventListener('click', () => {
            window.clearTimeout(rowClickTimer);
            rowClickTimer = window.setTimeout(() => showDetail(result), 220);
        });
        row.addEventListener('dblclick', () => {
            window.clearTimeout(rowClickTimer);
            if (result.directory.is_directory) openDirectory(result.directory.path);
        });
        row.addEventListener('keydown', (event) => {
            if (event.key === 'Enter') showDetail(result);
        });
        row.addEventListener('contextmenu', (event) => {
            event.preventDefault();
            openContextMenu(event.clientX, event.clientY, buildRowMenu(result));
        });

        const name = createElement('span', 'file-name');
        if (result.uiState === 'analyzing') {
            name.append(createElement('span', 'row-spinner'));
        } else if (result.uiState === 'queued') {
            name.append(createElement('span', 'row-queue-dot', '◦'));
        } else {
            name.append(createElement('span', `entry-icon ${result.directory.is_directory ? 'folder' : 'file'}`));
        }
        if (result.is_whitelisted) {
            const whitelistShield = createElement('span', 'whitelist-shield', '♢');
            whitelistShield.title = '已保护：删除时需要额外确认';
            name.append(whitelistShield);
        }
        if (result.cache_stale) {
            const staleIndicator = createElement('span', 'cache-stale-indicator', '◷');
            staleIndicator.title = '缓存可能已过期：目录内容、模型或分析版本已发生变化';
            name.append(staleIndicator);
        }
        const nameCopy = createElement('span', 'entry-name-copy');
        nameCopy.append(
            createElement('span', 'directory-name', result.directory.name),
            createElement('span', 'entry-kind', result.directory.is_directory ? '目录' : '文件'),
        );
        name.append(nameCopy);
        const purposeText = result.uiState === 'queued'
            ? '等待分析…'
            : result.uiState === 'analyzing'
                ? '正在分析…'
                : (result.verdict.purpose || '尚未分析');
        const purpose = createElement('span', 'file-purpose', purposeText);
        if (result.uiState === 'queued' || result.uiState === 'analyzing') purpose.classList.add('file-purpose-pending');
        const adviceKind = impactKind(result.verdict);
        const usage = createElement('span', `directory-state advice-${adviceKind}`);
        usage.title = result.verdict.delete_effect || impactLabel(result.verdict);
        usage.append(createElement('span', 'suggestion-dot'), createElement('span', 'advice-label', adviceShortLabel(result.verdict)));
        const sizeText = result.sizeState === 'precise'
            ? formatSize(result.directory.size)
            : result.sizeState === 'calculating'
                ? '计算中…'
                : '未统计';
        const size = createElement('span', 'directory-size', sizeText);
        if (result.sizeState === 'calculating') size.classList.add('directory-size-calculating');
        const actions = createElement('span', 'row-actions');
        const analyzeBusy = result.uiState === 'analyzing' || result.uiState === 'queued';
        const analyzeButton = createElement('button', '', result.uiState === 'analyzing' ? '分析中…' : result.uiState === 'queued' ? '等待中…' : '分析');
        analyzeButton.type = 'button';
        analyzeButton.disabled = analyzeBusy;
        analyzeButton.addEventListener('click', (event) => {
            event.stopPropagation();
            analyzeDirectory(result);
        });
        const whitelistButton = createElement(
            'button',
            '',
            result.is_whitelisted ? '取消保护' : '保护',
        );
        whitelistButton.type = 'button';
        whitelistButton.addEventListener('click', (event) => {
            event.stopPropagation();
            toggleWhitelist(result);
        });
        const trashButton = createElement('button', 'trash-button', '清理');
        trashButton.type = 'button';
        trashButton.title = '移到废纸篓';
        trashButton.addEventListener('click', (event) => {
            event.stopPropagation();
            trashDirectory(result);
        });
        actions.append(analyzeButton, whitelistButton, trashButton);
        row.append(name, purpose, usage, size, actions);
        elements.directoryList.append(row);
    });
}

function showDetail(result) {
    state.detailOverride = null;
    state.selectedPath = result.directory.path;
    state.detailTab = 'basic';
    result.aiExplain = null; // 每次重新打开都清空即席问答（不保存）
    renderList();
    renderDetail();
    elements.detailModal.classList.add('show');
    elements.detailModal.setAttribute('aria-hidden', 'false');
}

function hideDetail() {
    elements.detailModal.classList.remove('show');
    elements.detailModal.setAttribute('aria-hidden', 'true');
    state.detailOverride = null;
}

// 应用内确认弹窗。替代 window.confirm —— Tauri WebView 默认不实现原生 confirm，会直接返回 false。
function confirmDialog({ title = '确认', message = '', confirmText = '继续', cancelText = '取消', danger = false }) {
    return new Promise((resolve) => {
        const overlay = createElement('div', 'confirm-modal show');
        const backdrop = createElement('div', 'confirm-backdrop');
        const box = createElement('div', 'confirm-box');
        box.setAttribute('role', 'dialog');
        box.setAttribute('aria-modal', 'true');
        const actions = createElement('div', 'confirm-actions');
        const cancel = createElement('button', '', cancelText);
        cancel.type = 'button';
        const ok = createElement('button', danger ? 'trash-button' : 'primary-button', confirmText);
        ok.type = 'button';
        let settled = false;
        const close = (value) => {
            if (settled) return;
            settled = true;
            overlay.remove();
            document.removeEventListener('keydown', onKey, true);
            resolve(value);
        };
        function onKey(event) {
            if (event.key === 'Escape') { event.stopPropagation(); close(false); }
            else if (event.key === 'Enter') { event.stopPropagation(); close(true); }
        }
        cancel.addEventListener('click', () => close(false));
        ok.addEventListener('click', () => close(true));
        backdrop.addEventListener('click', () => close(false));
        actions.append(cancel, ok);
        box.append(createElement('h2', 'confirm-title', title), createElement('p', 'confirm-message', message), actions);
        overlay.append(backdrop, box);
        document.body.append(overlay);
        document.addEventListener('keydown', onKey, true);
        ok.focus();
    });
}

let contextMenuEl = null;

function closeContextMenu() {
    if (!contextMenuEl) return;
    contextMenuEl.remove();
    contextMenuEl = null;
    document.removeEventListener('pointerdown', onContextMenuOutside, true);
    document.removeEventListener('keydown', onContextMenuKeydown, true);
    window.removeEventListener('blur', closeContextMenu);
    window.removeEventListener('resize', closeContextMenu);
}

function swallowNextClick(event) {
    event.stopPropagation();
    event.preventDefault();
    document.removeEventListener('click', swallowNextClick, true);
}

function onContextMenuOutside(event) {
    if (!contextMenuEl || contextMenuEl.contains(event.target)) return;
    closeContextMenu();
    // 左键点空白或其它行来关闭菜单时，吞掉紧随的 click，避免误触发行单击（打开详情）。
    if (event.button === 0) {
        document.removeEventListener('click', swallowNextClick, true);
        document.addEventListener('click', swallowNextClick, true);
    }
}

function onContextMenuKeydown(event) {
    if (event.key === 'Escape') {
        event.stopPropagation();
        closeContextMenu();
    }
}

function openContextMenu(x, y, items) {
    closeContextMenu();
    const menu = createElement('div', 'context-menu');
    menu.setAttribute('role', 'menu');
    items.forEach((item) => {
        if (item.separator) {
            menu.append(createElement('div', 'context-menu-separator'));
            return;
        }
        const button = createElement('button', `context-menu-item${item.danger ? ' danger' : ''}`, item.label);
        button.type = 'button';
        button.setAttribute('role', 'menuitem');
        button.disabled = Boolean(item.disabled);
        button.addEventListener('click', () => {
            closeContextMenu();
            item.action();
        });
        menu.append(button);
    });
    document.body.append(menu);
    contextMenuEl = menu;

    const rect = menu.getBoundingClientRect();
    const left = Math.max(8, Math.min(x, window.innerWidth - rect.width - 8));
    const top = Math.max(8, Math.min(y, window.innerHeight - rect.height - 8));
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;

    // Defer so the initiating contextmenu/pointer event doesn't immediately close it.
    setTimeout(() => document.addEventListener('pointerdown', onContextMenuOutside, true), 0);
    document.addEventListener('keydown', onContextMenuKeydown, true);
    window.addEventListener('blur', closeContextMenu);
    window.addEventListener('resize', closeContextMenu);
    elements.directoryList.addEventListener('scroll', closeContextMenu, { once: true });
}

function buildRowMenu(result) {
    const isDir = result.directory.is_directory;
    const items = [{ label: '查看详情', action: () => showDetail(result) }];
    if (isDir) {
        items.push({ label: '进入目录', action: () => openDirectory(result.directory.path) });
    }
    items.push({
        // 目录：直接在访达中打开该文件夹；文件：打开其所在目录并选中该文件。
        label: isDir ? '在访达中打开' : '打开所在目录',
        action: async () => {
            try {
                await invoke('open_in_file_manager', { path: result.directory.path, reveal: !isDir });
            } catch (error) {
                elements.scanStatus.textContent = `打开失败：${String(error)}`;
            }
        },
    });
    items.push(
        {
            label: result.sizeState === 'calculating' ? '正在计算大小…' : '计算大小',
            disabled: result.sizeState === 'calculating',
            action: () => calculateDirectorySize(result),
        },
        {
            label: '复制路径',
            action: async () => {
                await navigator.clipboard.writeText(result.directory.path);
                elements.scanStatus.textContent = '路径已复制';
            },
        },
        { separator: true },
        { label: result.is_whitelisted ? '取消保护' : '保护', action: () => toggleWhitelist(result) },
        { separator: true },
        { label: '移到废纸篓', danger: true, action: () => trashDirectory(result) },
    );
    return items;
}

function openDirectory(path) {
    showDirectoryPanel();
    const match = [...document.querySelectorAll('.location-nav [data-location]')]
        .find((button) => button.dataset.location === path) || null;
    setActiveNav(match);
    scanDirectories(path);
}

function renderBreadcrumb(path) {
    const normalized = path || '~';
    const isHomePath = normalized === '~' || normalized.startsWith('~/');
    const parts = normalized.replace(/^~\/?/, '').split('/').filter(Boolean);
    const nodes = [];
    const root = createElement('button', 'breadcrumb-item', isHomePath ? '个人目录' : '/');
    root.type = 'button';
    root.addEventListener('click', () => openDirectory(isHomePath ? '~' : '/'));
    nodes.push(root);
    parts.forEach((part, index) => {
        nodes.push(createElement('span', 'breadcrumb-separator', '›'));
        const button = createElement('button', 'breadcrumb-item', part);
        button.type = 'button';
        const prefix = isHomePath ? '~/' : '/';
        button.addEventListener('click', () => openDirectory(`${prefix}${parts.slice(0, index + 1).join('/')}`));
        nodes.push(button);
    });
    elements.currentLocation.replaceChildren(...nodes);
}

function selectedResult() {
    return state.detailOverride
        || state.results.find((result) => result.directory.path === state.selectedPath)
        || null;
}

function showStoredDetail({ name, path, verdict, isWhitelisted, isConfirmed = false }) {
    state.detailOverride = {
        storedOnly: true,
        isConfirmed,
        directory: {
            path: path || name,
            name,
            is_directory: true,
            size: 0,
            last_modified: new Date().toISOString(),
            top_level_samples: [],
            bundle_id_hint: null,
        },
        verdict: verdict || {
            owner: null, purpose: '该保护记录暂时没有可引用的分析详情。', deletable: 'unknown',
            confidence: null, source: 'unknown', reason: '用户保护记录', is_residue: null,
        },
        is_whitelisted: Boolean(isWhitelisted),
        uiState: 'ready',
        sizeState: 'idle',
    };
    state.selectedPath = null;
    renderDetail();
    elements.detailModal.classList.add('show');
    elements.detailModal.setAttribute('aria-hidden', 'false');
}

function consequenceText(verdict) {
    if (verdict.delete_effect) return verdict.delete_effect;
    if (verdict.reason) return verdict.reason;
    return {
        safe: '删除后通常可由所属应用重新生成。',
        caution: '删除可能影响本地配置或登录状态，请先确认用途。',
        never: '该目录包含重要数据，不建议删除。',
        unknown: '可删性尚未确认，请先分析。',
    }[verdict.deletable] || '可删性尚未确认，请先分析。';
}

function renderDetail() {
    elements.detailPanel.replaceChildren();
    const result = selectedResult();
    if (!result) {
        const empty = createElement('div', 'detail-empty');
        empty.append(createElement('strong', '', '选择一个目录'), createElement('span', '', '用途说明和删除后果会显示在这里。'));
        elements.detailPanel.append(empty);
        return;
    }

    const { directory, verdict } = result;
    const content = createElement('article', 'detail-content');
    const heading = createElement('div', 'detail-heading');
    const titleGroup = createElement('div');
    titleGroup.append(createElement('h1', '', directory.name));
    const pathButton = createElement('button', 'path-button', directory.path);
    pathButton.type = 'button';
    pathButton.title = '复制完整路径';
    pathButton.addEventListener('click', async () => {
        await navigator.clipboard.writeText(directory.path);
        elements.scanStatus.textContent = '路径已复制';
    });
    titleGroup.append(pathButton);
    const badges = createElement('div', 'semantic-badges');
    badges.append(
        createElement('span', `semantic-badge usage-${usageKind(result)}`, `使用状态：${usageLabel(result)}`),
        createElement('span', `semantic-badge impact-${impactKind(verdict)}`, `删除影响：${impactLabel(verdict)}`),
    );
    heading.append(titleGroup, badges);

    const purpose = createElement('section', 'purpose-section');
    purpose.append(createElement('h2', '', '用途说明'), createElement('p', 'purpose-text', verdict.purpose || '尚未分析该目录的用途。'));

    const consequence = createElement('section', `consequence impact-${impactKind(verdict)}`);
    consequence.append(createElement('span', 'consequence-label', '删除后果'), createElement('p', '', consequenceText(verdict)));

    const info = createElement('dl', 'metadata');
    const rows = [
        ['归属', verdict.owner || '未知'], ['大小', formatSize(directory.size)],
        ['最后修改', formatDate(directory.last_modified)], ['判定来源', sourceLabel(verdict.source)],
    ];
    if (verdict.confidence !== null && verdict.confidence !== undefined) {
        rows.push(['置信度', `${Math.round(verdict.confidence * 100)}%`]);
    }
    if (verdict.reason) rows.push(['判断依据', verdict.reason]);
    if (result.cache_stale) rows.push(['缓存状态', '可能已过期，目录、模型或分析版本已发生变化']);
    if (result.analysisDurationMs !== undefined) {
        rows.push(['AI 耗时', `${(result.analysisDurationMs / 1000).toFixed(2)} 秒`]);
    }
    rows.push(['用户保护', result.is_whitelisted ? '已保护' : '未保护']);
    rows.forEach(([label, value]) => {
        info.append(createElement('dt', '', label), createElement('dd', '', value));
    });

    let ruleConfirmation = null;
    if (verdict.source === 'ai') {
        ruleConfirmation = createElement('section', 'rule-confirmation');
        const ruleCopy = createElement('div', 'rule-confirmation-copy');
        const canConfirm = verdict.source !== 'unknown' && result.uiState !== 'analyzing';
        ruleCopy.append(
            createElement('h3', '', '确认分析'),
            createElement(
                'p',
                '',
                canConfirm
                    ? '结果已经自动缓存；确认表示你认可并锁定这条判断。'
                    : '请先完成目录分析，再保存为已确认分析。',
            ),
        );
        const confirmButton = createElement('button', 'primary-button', '确认分析');
        confirmButton.type = 'button';
        confirmButton.disabled = !canConfirm;
        confirmButton.addEventListener('click', () => confirmAnalysis(result));
        ruleConfirmation.append(ruleCopy, confirmButton);
    } else if (verdict.source === 'cache' && verdict.locked) {
        ruleConfirmation = createElement('section', 'rule-confirmation confirmed');
        const ruleCopy = createElement('div', 'rule-confirmation-copy');
        ruleCopy.append(
            createElement('h3', '', '已确认分析'),
            createElement('p', '', '这条缓存已被你认可并锁定。'),
        );
        ruleConfirmation.append(ruleCopy);
        if (!result.storedOnly || result.isConfirmed) {
            const removeRuleButton = createElement('button', '', '取消确认');
            removeRuleButton.type = 'button';
            removeRuleButton.addEventListener('click', () => removeFromRuleLibrary(result));
            ruleConfirmation.append(removeRuleButton);
        }
    } else if (verdict.source === 'cache') {
        ruleConfirmation = createElement('section', 'rule-confirmation');
        const ruleCopy = createElement('div', 'rule-confirmation-copy');
        ruleCopy.append(
            createElement('h3', '', result.cache_stale ? '缓存可能已过期' : '已自动缓存'),
            createElement('p', '', result.cache_stale ? '旧结果仍保留；可重新检测，或认可并锁定当前判断。' : '无需再次调用 AI；你也可以认可并锁定这条判断。'),
        );
        const confirmButton = createElement('button', 'primary-button', '认可并锁定');
        confirmButton.type = 'button';
        confirmButton.addEventListener('click', () => confirmAnalysis(result));
        ruleConfirmation.append(ruleCopy, confirmButton);
    }

    let debugPanel = null;
    if (result.debug) {
        debugPanel = createElement('details', 'debug-panel');
        debugPanel.append(
            createElement('summary', '', 'AI 调试'),
            createDebugBlock('实际发送的 Prompt', result.debug.prompt),
            createDebugBlock('模型原始返回', result.debug.raw_response),
        );
    }

    const actions = createElement('div', 'detail-actions');
    const analyzeButton = createElement('button', '', result.uiState === 'analyzing' ? '分析中…' : '重新检测');
    analyzeButton.type = 'button';
    analyzeButton.disabled = result.uiState === 'analyzing';
    analyzeButton.addEventListener('click', () => analyzeDirectory(result));
    actions.append(analyzeButton);

    const whitelistButton = createElement('button', '', result.is_whitelisted ? '取消保护' : '保护');
    whitelistButton.type = 'button';
    whitelistButton.addEventListener('click', () => toggleWhitelist(result));
    actions.append(whitelistButton);

    content.append(heading);

    // 「基本信息」Tab 内容
    const basicBody = createElement('div', 'detail-tab-body');
    if (ruleConfirmation) basicBody.append(ruleConfirmation);
    basicBody.append(purpose, consequence, info);
    if (debugPanel) basicBody.append(debugPanel);
    if (!result.storedOnly) basicBody.append(actions);

    // 保护/已识别记录：无实时路径动作，只展示基本信息，不提供 AI 详解 Tab。
    if (result.storedOnly) {
        content.append(basicBody);
        elements.detailPanel.append(content);
        return;
    }

    // 「AI 详解」Tab 内容
    const aiBody = createElement('div', 'detail-tab-body');
    aiBody.append(createAiExplainSection(result));

    const tabBar = createElement('div', 'detail-tabs');
    const basicTab = createElement('button', 'detail-tab', '基本信息');
    const aiTab = createElement('button', 'detail-tab', 'AI 详解');
    basicTab.type = 'button';
    aiTab.type = 'button';
    const applyTab = () => {
        const isBasic = state.detailTab !== 'ai';
        basicTab.classList.toggle('active', isBasic);
        aiTab.classList.toggle('active', !isBasic);
        basicBody.hidden = !isBasic;
        aiBody.hidden = isBasic;
    };
    basicTab.addEventListener('click', () => { state.detailTab = 'basic'; applyTab(); });
    aiTab.addEventListener('click', () => { state.detailTab = 'ai'; applyTab(); });
    tabBar.append(basicTab, aiTab);

    content.append(tabBar, basicBody, aiBody);
    applyTab();
    elements.detailPanel.append(content);
}

function updateSummary() {
    const analyzed = state.results.filter((result) => result.verdict.source !== 'unknown').length;
    elements.sizeInfo.textContent = `已扫描 ${state.results.length} 项 · 已分析 ${analyzed} 项`;
}

function applyColumnPreferences() {
    document.getElementById('app').classList.toggle('hide-size-column', !state.showSizeColumn);
    elements.showSizeColumn.checked = state.showSizeColumn;
}

function render() {
    renderList();
    renderDetail();
    updateSummary();
}

async function scanDirectories(pathOverride) {
    const path = typeof pathOverride === 'string' ? pathOverride : state.currentPath;
    if (!path) return;
    // 切换目录时中止仍在进行的批量分析，避免继续更新已离开的结果。
    if (isBatchAnalyzing) analysisAbort = true;
    const requestId = ++scanRequestId;
    state.currentPath = path;
    state.results = [];
    state.selectedPath = null;
    state.isScanning = true;
    state.scanError = null;
    renderBreadcrumb(path);
    render();
    elements.scanBtn.disabled = true;
    hideDetail();
    elements.scanBtn.textContent = '扫描中…';
    elements.scanStatus.textContent = `正在扫描 ${path}`;
    try {
        const response = await invoke('scan_paths', { paths: [path] });
        if (requestId !== scanRequestId) return;
        state.currentPath = response.current_path || path;
        renderBreadcrumb(state.currentPath);
        state.results = response.results.map((result) => {
            const cached = state.resultCache.get(result.directory.path);
            if (!cached) return {
                ...result,
                uiState: 'ready',
                sizeState: result.directory.is_directory ? 'idle' : 'precise',
            };
            // 缓存里可能残留切走前的临时状态；无实际任务在跑，需复位，否则回来会永远「计算中/分析中」。
            const uiState = cached.uiState === 'analyzing' || cached.uiState === 'queued' ? 'ready' : cached.uiState;
            const sizeState = cached.sizeState === 'calculating' ? 'idle' : cached.sizeState;
            return {
                ...result,
                verdict: cached.verdict,
                debug: cached.debug,
                analysisDurationMs: cached.analysisDurationMs,
                uiState,
                sizeState,
                directory: { ...result.directory, size: cached.directory.size },
                is_whitelisted: result.is_whitelisted,
            };
        });
        state.results.forEach(cacheResult);
        state.hasScanned = true;
        state.isScanning = false;
        state.selectedPath = null;
        render();
        elements.scanStatus.textContent = response.error_count
            ? `扫描完成，${response.error_count} 项无法访问`
            : `扫描完成，共 ${state.results.length} 项`;
        calculateCurrentSizes();
    } catch (error) {
        if (requestId !== scanRequestId) return;
        state.isScanning = false;
        state.scanError = String(error);
        elements.scanStatus.textContent = `扫描失败：${state.scanError}`;
        render();
    } finally {
        if (requestId === scanRequestId) {
            elements.scanBtn.disabled = false;
            elements.scanBtn.textContent = '扫描';
        }
    }
}

async function calculateCurrentSizes() {
    const scannedPath = state.currentPath;
    const targets = state.results.filter((result) => result.sizeState !== 'precise');
    if (targets.length === 0) return;
    targets.forEach((result) => { result.sizeState = 'calculating'; });
    renderList();
    try {
        const sizes = await invoke('calculate_sizes', { dirs: targets.map((result) => result.directory) });
        targets.forEach((result, index) => {
            result.directory.size = sizes[index] ?? result.directory.size;
            result.sizeState = 'precise';
            cacheResult(result);
        });
        if (state.currentPath === scannedPath) {
            elements.scanStatus.textContent = `扫描完成，已精准统计 ${targets.length} 个目录大小`;
            render();
        }
    } catch (error) {
        targets.forEach((result) => { result.sizeState = 'idle'; });
        if (state.currentPath === scannedPath) {
            elements.scanStatus.textContent = `自动统计大小失败：${String(error)}`;
            renderList();
        }
    }
}

async function calculateDirectorySize(result) {
    result.sizeState = 'calculating';
    renderList();
    try {
        const sizes = await invoke('calculate_sizes', { dirs: [result.directory] });
        result.directory.size = sizes[0] ?? result.directory.size;
        result.sizeState = 'precise';
        cacheResult(result);
        elements.scanStatus.textContent = `${result.directory.name} 大小计算完成`;
        render();
    } catch (error) {
        result.sizeState = 'idle';
        elements.scanStatus.textContent = `大小统计失败：${String(error)}`;
        renderList();
    }
}

async function analyzeCurrentDirectory() {
    // 分析进行中时，此按钮充当「停止」。
    if (isBatchAnalyzing) {
        analysisAbort = true;
        elements.analyzeAllBtn.textContent = '正在停止…';
        elements.analyzeAllBtn.disabled = true;
        return;
    }

    // 重新分析当前目录：跳过已确认锁定的结果与命中内置规则的项（这两类无需再问 AI），
    // 其余（待分析 / 已缓存 / 过期）都会重新调用 AI，因此可反复重跑。
    const pending = state.results.filter(
        (result) => !result.verdict.locked
            && result.verdict.source !== 'local_rule'
            && result.uiState !== 'analyzing',
    );
    if (pending.length === 0) {
        elements.scanStatus.textContent = '当前目录没有可分析的项目（已确认锁定或命中内置规则的会跳过）';
        return;
    }

    const modelName = state.config.model || '当前模型';
    const concurrency = state.analyzeConcurrency;
    const confirmed = await confirmDialog({
        title: '确认分析当前目录',
        message: `将调用 AI 模型「${modelName}」逐个分析 ${pending.length} 个目录（并发 ${concurrency}）。\n\n此操作会消耗 API token 并产生相应费用。已确认锁定、命中内置规则的项会自动跳过。\n\n确定开始吗？`,
        confirmText: '开始分析',
    });
    if (!confirmed) return;

    isBatchAnalyzing = true;
    analysisAbort = false;
    elements.analyzeAllBtn.textContent = '停止分析';

    const total = pending.length;
    // 先把待分析项全部标记为「等待分析」，让用户看到完整排队。
    pending.forEach((result) => { result.uiState = 'queued'; });
    renderList();

    let done = 0;
    let ok = 0;
    let failed = 0;
    const startedAt = performance.now();
    const updateProgress = () => {
        let etaText = '';
        if (done > 0 && done < total) {
            const remainingMs = ((performance.now() - startedAt) / done) * (total - done);
            etaText = `，预计剩余 ${formatDuration(remainingMs)}`;
        }
        elements.scanStatus.textContent = `分析中 ${done}/${total}（成功 ${ok}，失败 ${failed}）${etaText}`;
    };
    updateProgress();

    const queue = pending.slice();
    const worker = async () => {
        while (queue.length && !analysisAbort) {
            const result = queue.shift();
            if (!result) break;
            result.uiState = 'analyzing';
            renderList();
            const success = await runAnalysis(result);
            done += 1;
            if (success) ok += 1;
            else failed += 1;
            updateProgress();
            renderList();
        }
    };
    const workers = Array.from({ length: Math.min(concurrency, total) }, worker);
    await Promise.all(workers);

    // 被中止而未开始的项仍是 queued，恢复为普通状态。
    pending.forEach((result) => { if (result.uiState === 'queued') result.uiState = 'ready'; });

    const wasAborted = analysisAbort;
    const skipped = total - done;
    isBatchAnalyzing = false;
    analysisAbort = false;
    elements.analyzeAllBtn.textContent = '分析当前目录';
    elements.analyzeAllBtn.disabled = false;
    const totalElapsed = formatDuration(performance.now() - startedAt);
    elements.scanStatus.textContent = wasAborted
        ? `已停止：成功 ${ok}，失败 ${failed}，未分析 ${skipped}，共 ${total} 项`
        : `分析完成：成功 ${ok}，失败 ${failed}，共 ${total} 项，耗时 ${totalElapsed}`;
    render();
}

async function confirmCurrentAnalyses() {
    const confirmable = state.results.filter((result) => result.verdict.source === 'ai');
    if (confirmable.length === 0) {
        elements.scanStatus.textContent = '当前列表没有待确认的 AI 分析结果';
        return;
    }
    elements.confirmAllBtn.disabled = true;
    try {
        const count = await invoke('confirm_analyses', {
            entries: confirmable.map((result) => ({ key: result.verdict.key, name: result.directory.name, verdict: result.verdict })),
        });
        confirmable.forEach((result) => {
            result.verdict = { ...result.verdict, source: 'cache', locked: true };
            cacheResult(result);
        });
        elements.scanStatus.textContent = `已确认 ${count} 条 AI 分析结果`;
        render();
    } catch (error) {
        elements.scanStatus.textContent = `批量确认失败：${String(error)}`;
    } finally {
        elements.confirmAllBtn.disabled = false;
    }
}

function parentPath(path) {
    const normalized = path.replace(/\/$/, '');
    if (normalized === '~' || normalized === '/') return normalized;
    const index = normalized.lastIndexOf('/');
    if (index < 0) return '~';
    if (index === 0) return '/';
    return normalized.slice(0, index);
}

function openParentDirectory() {
    openDirectory(parentPath(state.currentPath));
}

// 分析单个目录并就地更新该 result；返回是否成功。不负责渲染或状态栏文案，交给调用方。
async function runAnalysis(result) {
    result.uiState = 'analyzing';
    const startedAt = performance.now();
    try {
        const response = await invoke('reanalyze_directory', { dir: result.directory });
        result.verdict = response.verdict;
        result.cache_stale = false;
        result.debug = response.debug;
        result.analysisDurationMs = performance.now() - startedAt;
        result.uiState = 'ready';
        result.analysisError = null;
        cacheResult(result);
        return true;
    } catch (error) {
        result.uiState = 'failed';
        result.analysisError = String(error);
        cacheResult(result);
        return false;
    }
}

async function analyzeDirectory(result) {
    result.uiState = 'analyzing';
    render();
    elements.scanStatus.textContent = `正在分析 ${result.directory.name}`;
    const success = await runAnalysis(result);
    elements.scanStatus.textContent = success
        ? `${result.directory.name} 分析完成，耗时 ${(result.analysisDurationMs / 1000).toFixed(2)} 秒`
        : `分析失败：${result.analysisError}`;
    render();
}

async function toggleWhitelist(result) {
    try {
        result.is_whitelisted = await invoke('set_whitelist', {
            path: result.directory.path,
            enabled: !result.is_whitelisted,
            verdict: null,
        });
        cacheResult(result);
        elements.scanStatus.textContent = result.is_whitelisted
            ? `${result.directory.name} 已保护`
            : `${result.directory.name} 已取消保护`;
        render();
    } catch (error) {
        elements.scanStatus.textContent = `保护状态更新失败：${String(error)}`;
    }
}

async function confirmAnalysis(result) {
    try {
        await invoke('confirm_analysis', { name: result.verdict.key, verdict: result.verdict });
        result.verdict = { ...result.verdict, source: 'cache', locked: true };
        cacheResult(result);
        elements.scanStatus.textContent = `${result.directory.name} 已确认，下次将直接使用该结果`;
        render();
    } catch (error) {
        elements.scanStatus.textContent = `确认结果失败：${String(error)}`;
    }
}

async function removeFromRuleLibrary(result) {
    try {
        await invoke('unlock_analysis', { key: result.verdict.key || result.directory.path });
        result.verdict.locked = false;
        cacheResult(result);
        elements.scanStatus.textContent = `${result.directory.name} 已取消确认`;
        if (result.storedOnly) {
            hideDetail();
            renderKnowledgeSettings();
        }
        render();
    } catch (error) {
        elements.scanStatus.textContent = `取消确认失败：${String(error)}`;
    }
}

async function trashDirectory(result) {
    const caution = result.verdict.deletable === 'caution' || result.verdict.is_residue;
    const ok = await confirmDialog({
        title: '移到废纸篓',
        message: caution
            ? `${consequenceText(result.verdict)}\n\n仍要将 ${result.directory.name} 移到废纸篓吗？`
            : `将 ${result.directory.name} 移到废纸篓？`,
        confirmText: '移到废纸篓',
        danger: true,
    });
    if (!ok) return;
    let forceProtected = false;
    if (result.is_whitelisted) {
        forceProtected = await confirmDialog({
            title: '该目录已保护',
            message: `${result.directory.name} 已被你保护。\n\n确定仍要移到废纸篓吗？`,
            confirmText: '仍然删除',
            danger: true,
        });
        if (!forceProtected) return;
    }
    try {
        await invoke('trash_directory', { path: result.directory.path, forceProtected });
        state.results = state.results.filter((item) => item.directory.path !== result.directory.path);
        state.selectedPath = state.results[0]?.directory.path || null;
        elements.scanStatus.textContent = `${result.directory.name} 已移到废纸篓`;
        render();
    } catch (error) {
        elements.scanStatus.textContent = `操作失败：${String(error)}`;
    }
}

async function loadConfig() {
    try {
        state.config = await invoke('get_config');
        state.customLocations = state.config.custom_locations || [];
        renderCustomLocations();
    } catch (error) {
        elements.scanStatus.textContent = `配置读取失败：${String(error)}`;
    }
}

async function loadAppVersion() {
    try {
        const version = await invoke('get_app_version');
        elements.aboutVersion.textContent = `v${version}`;
    } catch (error) {
        elements.aboutVersion.textContent = '未知';
    }
}

async function loadSystemInfo() {
    try {
        const systemInfo = await invoke('get_system_info');
        elements.aboutBuildType.textContent = systemInfo.build_type;
        elements.aboutPlatform.textContent = `${systemInfo.platform} (${systemInfo.arch})`;
        elements.aboutOsVersion.textContent = systemInfo.os_version;
    } catch (error) {
        console.error('加载系统信息失败:', error);
    }
}

async function checkForUpdates() {
    try {
        elements.checkUpdateBtn.disabled = true;
        elements.checkUpdateBtn.textContent = '检查中…';
        elements.updateStatus.textContent = '正在检查…';
        elements.latestVersion.textContent = '检查中…';
        elements.downloadUpdateBtn.style.display = 'none';

        // 使用 Tauri updater 插件检查更新
        const { check } = await import('@tauri-apps/plugin-updater');
        const { getVersion } = await import('@tauri-apps/api/app');

        const currentVersion = await getVersion();
        const update = await check({ timeout: 30000 });

        if (!update) {
            elements.updateStatus.textContent = '当前版本已是最新';
            elements.latestVersion.textContent = `v${currentVersion} (最新)`;
            return;
        }

        elements.latestVersion.textContent = `v${update.version}`;
        elements.updateStatus.textContent = `发现新版本 v${update.version}`;

        elements.downloadUpdateBtn.style.display = 'inline-block';
        elements.downloadUpdateBtn.textContent = '下载并安装';
        elements.downloadUpdateBtn.onclick = async () => {
            try {
                elements.downloadUpdateBtn.disabled = true;
                elements.downloadUpdateBtn.textContent = '下载中…';
                elements.updateStatus.textContent = '正在下载更新…';

                let downloaded = 0;

                // 使用 downloadAndInstall 一次性下载并安装
                await update.downloadAndInstall((event) => {
                    if (event.event === 'Progress') {
                        downloaded += event.data.chunkLength;
                        elements.updateStatus.textContent =
                            `下载中: ${formatSize(downloaded)}`;
                    }
                });

                elements.updateStatus.textContent = '安装完成，请手动重启应用';
                elements.downloadUpdateBtn.textContent = '安装完成';
            } catch (error) {
                elements.updateStatus.textContent = `更新失败：${String(error)}`;
                elements.downloadUpdateBtn.disabled = false;
                elements.downloadUpdateBtn.textContent = '重试';
            }
        };
    } catch (error) {
        elements.updateStatus.textContent = `检查失败：${String(error)}`;
        elements.latestVersion.textContent = '检查失败';
        elements.downloadUpdateBtn.style.display = 'none';
    } finally {
        elements.checkUpdateBtn.disabled = false;
        elements.checkUpdateBtn.textContent = '检查更新';
    }
}

function setActiveNav(target) {
    document.querySelectorAll('.location-nav .location-item').forEach((btn) => {
        btn.classList.toggle('active', target != null && btn === target);
    });
}

// ——— 自定义扫描位置（持久化到后端配置文件，与 API Key/模型同处，更新后不丢）———
function customLocationName(path) {
    const trimmed = path.replace(/\/+$/, '');
    return trimmed.split('/').filter(Boolean).pop() || trimmed || path;
}

function renderCustomLocations() {
    const nav = elements.scanLocationNav;
    const addBtn = elements.customLocationBtn;
    nav.querySelectorAll('.custom-location-item').forEach((el) => el.remove());
    state.customLocations.forEach((path) => {
        const item = createElement('button', 'location-item custom-location-item');
        item.type = 'button';
        item.dataset.location = path;
        item.title = path;
        const remove = createElement('span', 'location-remove', '×');
        remove.title = '从侧边栏移除此位置';
        remove.addEventListener('click', (event) => {
            event.stopPropagation();
            removeCustomLocation(path);
        });
        const icon = createElement('span', 'nav-icon');
        icon.innerHTML = NAV_ICONS.folder;
        item.append(
            icon,
            createElement('span', 'location-item-name', customLocationName(path)),
            remove,
        );
        item.addEventListener('click', () => openDirectory(path));
        item.addEventListener('contextmenu', (event) => {
            event.preventDefault();
            openContextMenu(event.clientX, event.clientY, [
                { label: '在此扫描', action: () => openDirectory(path) },
                { separator: true },
                { label: '从侧边栏移除', danger: true, action: () => removeCustomLocation(path) },
            ]);
        });
        nav.insertBefore(item, addBtn);
    });
}

async function addCustomLocationFlow() {
    try {
        const path = await invoke('pick_directory');
        if (!path) return;
        state.customLocations = await invoke('add_custom_location', { path });
        renderCustomLocations();
        openDirectory(path);
    } catch (error) {
        elements.scanStatus.textContent = `添加扫描位置失败：${String(error)}`;
    }
}

async function removeCustomLocation(path) {
    try {
        state.customLocations = await invoke('remove_custom_location', { path });
        renderCustomLocations();
        elements.scanStatus.textContent = `已从侧边栏移除：${customLocationName(path)}`;
    } catch (error) {
        elements.scanStatus.textContent = `移除失败：${String(error)}`;
    }
}

function showBrowserView() {
    elements.settingsView.hidden = true;
    elements.browserView.hidden = false;
    document.getElementById('app').classList.remove('settings-mode');
}

function showHomePanel() {
    showBrowserView();
    state.browserPanel = 'home';
    elements.homeContent.hidden = false;
    elements.directoryContent.hidden = true;
    document.getElementById('app').classList.add('browser-home');
    setActiveNav(elements.homeBtn);
    updateHomeStats();
}

function showDirectoryPanel() {
    showBrowserView();
    state.browserPanel = 'directory';
    elements.homeContent.hidden = true;
    elements.directoryContent.hidden = false;
    document.getElementById('app').classList.remove('browser-home');
}

function updateHomeStats() {
    const cached = [...state.resultCache.values()];
    const analyzed = cached.filter((r) => r.verdict.source !== 'unknown').length;
    const cleanable = cached.filter((r) => ['safe', 'caution'].includes(r.verdict.deletable) || r.verdict.is_residue).length;
    elements.statScanned.textContent = cached.length;
    elements.statAnalyzed.textContent = analyzed;
    elements.statCleanable.textContent = cleanable;
}

function showSettings() {
    elements.apiKeyInput.value = '';
    elements.providerSelect.value = state.config.provider;
    elements.baseUrlInput.value = state.config.base_url;
    populateModelSelect([state.config.model], state.config.model);
    elements.modelStatus.textContent = '';
    elements.concurrencyInput.value = String(state.analyzeConcurrency);
    elements.configPath.textContent = state.config.config_path || '';
    elements.keyStatus.textContent = state.config.has_api_key
        ? 'API Key 已保存在本机 DirDetective 配置文件'
        : '尚未配置 API Key';
    elements.browserView.hidden = true;
    elements.settingsView.hidden = false;
    document.getElementById('app').classList.remove('browser-home');
    document.getElementById('app').classList.add('settings-mode');
    switchSettingsTab('basic');
    renderWhitelistSettings();
    renderKnowledgeSettings();
    loadAppVersion();
}

function switchSettingsTab(tab) {
    document.querySelectorAll('[data-settings-tab]').forEach((button) => {
        button.classList.toggle('active', button.dataset.settingsTab === tab);
    });
    document.querySelectorAll('[data-settings-panel]').forEach((panel) => {
        panel.classList.toggle('active', panel.dataset.settingsPanel === tab);
    });

    // 切换到关于面板时加载系统信息
    if (tab === 'about') {
        loadAppVersion();
        loadSystemInfo();
        // 重置更新状态
        elements.updateStatus.textContent = '-';
        elements.latestVersion.textContent = '未检查';
        elements.downloadUpdateBtn.style.display = 'none';
    } else if (tab === 'guard') {
        renderGuardSettings();
    } else if (tab === 'ruleset') {
        renderRulesetSettings();
    }
}

async function renderRulesetSettings() {
    const box = elements.rulesetSettingsContent;
    box.replaceChildren(createElement('div', 'list-empty', '正在读取规则库…'));
    try {
        const info = await invoke('get_ruleset_info');
        box.replaceChildren();

        const bar = createElement('div', 'ruleset-bar');
        const meta = createElement('div', 'ruleset-meta');
        meta.append(
            createElement('span', 'ruleset-version', `版本 v${info.version}`),
            createElement('span', 'ruleset-count', `${info.count} 条规则 · ${info.os}`),
        );
        const updateBtn = createElement('button', 'primary-button', '检查更新');
        updateBtn.type = 'button';
        updateBtn.addEventListener('click', async () => {
            updateBtn.disabled = true;
            const label = updateBtn.textContent;
            updateBtn.textContent = '检查中…';
            try {
                const r = await invoke('update_rules');
                if (r.updated) {
                    elements.scanStatus.textContent = `规则库已更新：v${r.local_version} → v${r.remote_version}（${r.count} 条）`;
                    renderRulesetSettings();
                    return;
                }
                elements.scanStatus.textContent = `规则库已是最新：本地 v${r.local_version}，远程 v${r.remote_version}`;
            } catch (error) {
                elements.scanStatus.textContent = `检查更新失败：${String(error)}`;
            } finally {
                updateBtn.disabled = false;
                updateBtn.textContent = label;
            }
        });
        bar.append(meta, updateBtn);
        box.append(bar, createElement('p', 'field-note', `规则文件：${info.path}`));

        const list = createElement('div', 'ruleset-list');
        info.rules.forEach((rule) => {
            const row = createElement('div', 'ruleset-row');
            const head = createElement('div', 'ruleset-row-head');
            head.append(
                createElement('strong', 'ruleset-name', rule.path),
                createElement('span', `ruleset-tag advice-${impactKind({ deletable: rule.deletable })}`, rule.deletable),
            );
            row.append(head);
            if (rule.owner) row.append(createElement('span', 'ruleset-owner', rule.owner));
            if (rule.purpose) row.append(createElement('p', 'ruleset-purpose', rule.purpose));
            list.append(row);
        });
        box.append(list);
    } catch (error) {
        box.replaceChildren(createElement('div', 'list-empty', `读取失败：${String(error)}`));
    }
}

async function renderGuardSettings() {
    const container = elements.guardSettingsContent;
    container.replaceChildren(createElement('div', 'list-empty', '正在读取防护规则…'));
    try {
        const rules = await invoke('get_trash_guard_rules');
        container.replaceChildren();

        const intro = createElement('div', 'guard-intro');
        intro.append(
            createElement('p', '', '清理（移到废纸篓）时，以下位置受保护、无法删除。防护分两种：系统目录“向下传染”（连同全部子项都保护），容器仅保护其自身（内部项目仍可清理）。'),
        );
        const tip = createElement('p', 'guard-tip', '');
        tip.append(
            createElement('strong', '', '注意：'),
            document.createTextNode('系统级 /Library 与你的家目录内 ~/Library 是两个不同位置——前者受保护，后者（如 ~/Library/Caches）可以清理。'),
        );
        intro.append(tip);
        container.append(intro);

        const makeGroup = (title, note, items, kind) => {
            const group = createElement('section', 'guard-group');
            group.append(createElement('h2', 'guard-group-title', title));
            if (note) group.append(createElement('p', 'guard-group-note', note));
            const list = createElement('div', 'guard-list');
            items.forEach((path) => {
                const row = createElement('div', `guard-row guard-${kind}`);
                row.append(
                    createElement('span', 'guard-path', path),
                    createElement('span', 'guard-badge', kind === 'prefix' ? '含全部子项' : '仅自身'),
                );
                list.append(row);
            });
            group.append(list);
            return group;
        };

        container.append(makeGroup(
            '系统目录（含全部子项）',
            '这些目录及其下的一切都不可删除。',
            rules.protected_prefixes,
            'prefix',
        ));
        container.append(makeGroup(
            '容器目录（仅保护自身）',
            '容器本身不可删除，但里面的项目可以清理。',
            rules.protected_containers,
            'container',
        ));
        container.append(makeGroup(
            '账户与根目录',
            '你的家目录本身及其所有上级目录都不可删除（家目录内的项目可以清理）。',
            [rules.home, '/Users', '/'],
            'container',
        ));
    } catch (error) {
        container.replaceChildren(createElement('div', 'list-empty', `读取失败：${String(error)}`));
    }
}

async function openDataDirectory() {
    try {
        await invoke('open_data_directory');
    } catch (error) {
        elements.keyStatus.textContent = `打开失败：${String(error)}`;
    }
}

function changeProvider() {
    const defaults = providerDefaults[elements.providerSelect.value] || providerDefaults.custom;
    elements.baseUrlInput.value = defaults.baseUrl;
    populateModelSelect(defaults.model ? [defaults.model] : [], defaults.model);
    elements.modelStatus.textContent = '';
    elements.keyStatus.textContent = '切换厂家后请填写或保存对应的 API Key';
}

function populateModelSelect(modelIds, selected) {
    const ids = [...new Set(modelIds.filter(Boolean))];
    elements.modelInput.replaceChildren(...ids.map((id) => {
        const option = document.createElement('option');
        option.value = id;
        option.textContent = id;
        return option;
    }));
    if (selected && !ids.includes(selected)) {
        const option = document.createElement('option');
        option.value = selected;
        option.textContent = selected;
        elements.modelInput.prepend(option);
    }
    elements.modelInput.value = selected || ids[0] || '';
}

async function fetchModels() {
    elements.fetchModelsBtn.disabled = true;
    elements.modelStatus.textContent = '正在获取模型列表…';
    try {
        const models = await invoke('fetch_models', {
            provider: elements.providerSelect.value,
            baseUrl: elements.baseUrlInput.value.trim(),
            apiKey: elements.apiKeyInput.value || null,
        });
        const current = elements.modelInput.value;
        populateModelSelect(models.map((model) => model.id), current || models[0]?.id);
        elements.modelStatus.textContent = `已获取 ${models.length} 个模型`;
    } catch (error) {
        elements.modelStatus.textContent = `获取失败：${String(error)}`;
    } finally {
        elements.fetchModelsBtn.disabled = false;
    }
}

function hideSettings() {
    if (state.browserPanel === 'directory') showDirectoryPanel();
    else showHomePanel();
}

async function renderWhitelistSettings() {
    elements.whitelistSettingsList.replaceChildren(createElement('div', 'list-empty', '正在读取已保护目录…'));
    try {
        const entries = await invoke('get_whitelist_entries');
        if (entries.length === 0) {
            elements.whitelistSettingsList.replaceChildren(createElement('div', 'list-empty', '暂无已保护目录'));
            return;
        }
        elements.whitelistSettingsList.replaceChildren(...entries.map((entry) => {
            const row = createElement('div', 'settings-data-row knowledge-row');
            const copy = createElement('div', 'knowledge-copy');
            copy.append(
                createElement('strong', '', entry.key.split('/').filter(Boolean).pop() || entry.key),
                createElement('span', '', entry.verdict?.owner || '未附带分析详情'),
                createElement('p', '', entry.verdict?.purpose || entry.key),
            );
            row.addEventListener('click', () => showStoredDetail({
                name: entry.key.split('/').filter(Boolean).pop() || entry.key,
                path: entry.key,
                verdict: entry.verdict,
                isWhitelisted: true,
            }));
            const removeButton = createElement('button', '', '移除');
            removeButton.type = 'button';
            removeButton.addEventListener('click', async (event) => {
                event.stopPropagation();
                await invoke('set_whitelist', { path: entry.key, enabled: false, verdict: null });
                const cached = [...state.resultCache.values()].find((item) => item.verdict.key === entry.key);
                if (cached) cached.is_whitelisted = false;
                state.results.forEach((result) => {
                    if (result.verdict.key === entry.key) result.is_whitelisted = false;
                });
                renderWhitelistSettings();
            });
            row.append(copy, removeButton);
            return row;
        }));
    } catch (error) {
        elements.whitelistSettingsList.replaceChildren(createElement('div', 'list-empty', `读取失败：${String(error)}`));
    }
}

async function renderKnowledgeSettings() {
    elements.knowledgeSettingsList.replaceChildren(createElement('div', 'list-empty', '正在读取已识别目录…'));
    try {
        const entries = await invoke('get_knowledge_entries');
        if (entries.length === 0) {
            elements.knowledgeSettingsList.replaceChildren(createElement('div', 'list-empty', '暂无已确认的分析结果'));
            return;
        }
        elements.knowledgeSettingsList.replaceChildren(...entries.map((entry) => {
            const row = createElement('div', 'settings-data-row knowledge-row');
            const copy = createElement('div', 'knowledge-copy');
            copy.append(
                createElement('strong', '', entry.name),
                createElement('span', '', entry.verdict.owner || '未知归属'),
                createElement('p', '', entry.verdict.purpose || '没有用途说明'),
            );
            row.addEventListener('click', () => showStoredDetail({
                name: entry.name,
                path: entry.key,
                verdict: { ...entry.verdict, source: 'cache' },
                isWhitelisted: false,
                isConfirmed: true,
            }));
            const removeButton = createElement('button', '', '移除');
            removeButton.type = 'button';
            removeButton.addEventListener('click', async (event) => {
                event.stopPropagation();
                await invoke('remove_knowledge_entry', { name: entry.key });
                state.resultCache.forEach((result) => {
                    if (result.verdict.key === entry.key && result.verdict.source === 'cache') {
                        result.verdict = {
                            key: entry.key, dir_name: result.directory.name, owner: null, purpose: '', delete_effect: '',
                            deletable: 'unknown', confidence: null, source: 'unknown', reason: '尚未分析',
                            evidence: [], is_residue: null, model_id: null,
                        };
                    }
                });
                renderKnowledgeSettings();
            });
            row.append(copy, removeButton);
            return row;
        }));
    } catch (error) {
        elements.knowledgeSettingsList.replaceChildren(createElement('div', 'list-empty', `读取失败：${String(error)}`));
    }
}

async function previewPrompt() {
    try {
        const prompt = await invoke('preview_ai_prompt', { dirs: state.results.map((result) => result.directory) });
        elements.detailPanel.replaceChildren(
            createElement('article', 'detail-content prompt-preview'),
        );
        const content = elements.detailPanel.firstElementChild;
        content.append(
            createElement('h1', '', '将发送给 AI 的内容'),
            createElement('p', 'field-note', '已排除个人目录，并仅保留允许的配置类文件名。'),
            createElement('pre', 'prompt-preview-content', prompt || '当前没有需要发送给 AI 的未知项目。'),
        );
        elements.detailModal.classList.add('show');
        elements.detailModal.setAttribute('aria-hidden', 'false');
    } catch (error) {
        elements.scanStatus.textContent = `预览失败：${String(error)}`;
    }
}

async function chooseDirectory() {
    try {
        const path = await invoke('pick_directory');
        if (path) openDirectory(path);
    } catch (error) {
        elements.scanStatus.textContent = `选择目录失败：${String(error)}`;
    }
}

async function saveSettings() {
    elements.saveSettingsBtn.disabled = true;
    try {
        state.config = await invoke('save_config_command', {
            config: {
                provider: elements.providerSelect.value,
                base_url: elements.baseUrlInput.value.trim(),
                model: elements.modelInput.value.trim(),
                api_key: elements.apiKeyInput.value || null,
            },
        });
        elements.keyStatus.textContent = state.config.has_api_key
            ? 'API Key 已保存在本机 DirDetective 配置文件'
            : '尚未配置 API Key';
        elements.scanStatus.textContent = '设置已保存';
    } catch (error) {
        elements.keyStatus.textContent = `保存失败：${String(error)}`;
    } finally {
        elements.saveSettingsBtn.disabled = false;
    }
}

function reportRuntimeError(error) {
    console.error(error);
    const status = elements.scanStatus || document.getElementById('scanStatus');
    if (status) status.textContent = `界面错误：${error?.message || String(error)}`;
}

function initializeApp() {
    if (document.documentElement.dataset.appInitialized === 'true') return;

    initLang();

    const requiredIds = [
        'homeBtn', 'homeContent', 'directoryContent', 'homeChooseDirBtn', 'homeScanCachesBtn',
        'statScanned', 'statAnalyzed', 'statCleanable',
        'scanBtn', 'chooseDirBtn', 'upBtn', 'analyzeAllBtn', 'previewPromptBtn', 'confirmAllBtn', 'filterSelect',
        'directoryList', 'detailPanel', 'detailModal', 'closeDetailBtn', 'currentLocation', 'resultCount',
        'scanStatus', 'sizeInfo', 'browserView', 'settingsView', 'backToBrowserBtn',
        'providerSelect', 'baseUrlInput', 'apiKeyInput', 'keyStatus', 'modelInput', 'modelStatus', 'fetchModelsBtn',
        'configPath', 'openDataDirBtn', 'customLocationBtn', 'scanLocationNav', 'sidebarSettingsBtn', 'showSizeColumn', 'concurrencyInput',
        'saveSettingsBtn', 'refreshWhitelistBtn', 'whitelistSettingsList',
        'refreshKnowledgeBtn', 'knowledgeSettingsList', 'guardSettingsContent', 'rulesetSettingsContent', 'languageSelect',
        'aboutVersion', 'aboutBuildType', 'aboutPlatform', 'aboutOsVersion',
        'latestVersion', 'updateStatus', 'checkUpdateBtn', 'downloadUpdateBtn',
    ];
    requiredIds.forEach((id) => { elements[id] = document.getElementById(id); });
    const missing = requiredIds.filter((id) => !elements[id]);
    if (missing.length > 0) {
        throw new Error(`缺少界面元素：${missing.join(', ')}`);
    }

    elements.scanBtn.addEventListener('click', scanDirectories);
    document.querySelector('.titlebar-drag').addEventListener('mousedown', (event) => {
        if (event.button === 0) invoke('start_window_drag').catch(reportRuntimeError);
    });
    elements.chooseDirBtn.addEventListener('click', chooseDirectory);
    elements.customLocationBtn.addEventListener('click', addCustomLocationFlow);
    elements.upBtn.addEventListener('click', openParentDirectory);
    elements.analyzeAllBtn.addEventListener('click', analyzeCurrentDirectory);
    elements.previewPromptBtn.addEventListener('click', previewPrompt);
    elements.confirmAllBtn.addEventListener('click', confirmCurrentAnalyses);
    elements.filterSelect.addEventListener('change', () => { state.filter = elements.filterSelect.value; render(); });
    document.querySelectorAll('[data-sort-key]').forEach((button) => {
        button.addEventListener('click', () => changeSort(button.dataset.sortKey));
    });
    elements.sidebarSettingsBtn.addEventListener('click', showSettings);
    elements.homeBtn.addEventListener('click', showHomePanel);
    elements.homeChooseDirBtn.addEventListener('click', chooseDirectory);
    elements.homeScanCachesBtn.addEventListener('click', () => openDirectory('~/Library/Caches'));
    document.querySelectorAll('[data-location]').forEach((button) => {
        button.addEventListener('click', () => openDirectory(button.dataset.location));
    });
    elements.showSizeColumn.addEventListener('change', () => {
        state.showSizeColumn = elements.showSizeColumn.checked;
        localStorage.setItem('showSizeColumn', String(state.showSizeColumn));
        applyColumnPreferences();
    });
    elements.concurrencyInput.addEventListener('change', () => {
        const value = Number(elements.concurrencyInput.value) || DEFAULT_ANALYZE_CONCURRENCY;
        state.analyzeConcurrency = value;
        localStorage.setItem('analyzeConcurrency', String(value));
    });
    elements.languageSelect.value = getLangPref();
    elements.languageSelect.addEventListener('change', () => {
        setLangPref(elements.languageSelect.value);
        applyStaticI18n();
        renderCustomLocations();
        render();
        if (document.querySelector('[data-settings-panel="guard"]').classList.contains('active')) {
            renderGuardSettings();
        }
    });
    elements.providerSelect.addEventListener('change', changeProvider);
    elements.fetchModelsBtn.addEventListener('click', fetchModels);
    elements.openDataDirBtn.addEventListener('click', openDataDirectory);
    elements.backToBrowserBtn.addEventListener('click', hideSettings);
    elements.refreshWhitelistBtn.addEventListener('click', renderWhitelistSettings);
    elements.refreshKnowledgeBtn.addEventListener('click', renderKnowledgeSettings);
    elements.checkUpdateBtn.addEventListener('click', checkForUpdates);
    document.querySelectorAll('[data-settings-tab]').forEach((button) => {
        button.addEventListener('click', () => switchSettingsTab(button.dataset.settingsTab));
    });
    elements.closeDetailBtn.addEventListener('click', hideDetail);
    elements.detailModal.querySelector('[data-close-detail]').addEventListener('click', hideDetail);
    elements.saveSettingsBtn.addEventListener('click', saveSettings);
    document.addEventListener('keydown', (event) => {
        if (event.key === 'Escape') {
            hideDetail();
            hideSettings();
        }
    });

    document.documentElement.dataset.appInitialized = 'true';
    document.querySelectorAll('.nav-icon[data-icon]').forEach((el) => {
        const svg = NAV_ICONS[el.dataset.icon];
        if (svg) el.innerHTML = svg;
    });
    applyStaticI18n();
    renderSortHeaders();
    applyColumnPreferences();
    renderCustomLocations();
    showHomePanel();
    loadConfig();
}

window.addEventListener('error', (event) => reportRuntimeError(event.error || event.message));
window.addEventListener('unhandledrejection', (event) => reportRuntimeError(event.reason));

try {
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', () => {
            try { initializeApp(); } catch (error) { reportRuntimeError(error); }
        }, { once: true });
    } else {
        initializeApp();
    }
} catch (error) {
    reportRuntimeError(error);
}
