// 轻量 i18n。zh 存原文作基准；t(key) 缺失时回退 zh 再回退 key —— 漏译只显示中文，绝不崩。
const translations = {
    zh: {
        'toolbar.up': '返回上级目录',
        'toolbar.scan': '扫描',
        'toolbar.chooseDir': '选择目录',
        'toolbar.analyzeAll': '分析当前目录',
        'toolbar.preview': '预览发送内容',
        'toolbar.confirmAll': '批量确认 AI 结果',
        'filter.aria': '筛选目录',
        'filter.all': '全部目录',
        'filter.actionable': '建议处理',
        'filter.unknown': '待分析',
        'filter.hideWhitelist': '隐藏已保护目录',
        'filter.whitelist': '仅已保护目录',
        'filter.hideHidden': '隐藏隐藏项',
        'filter.hidden': '仅隐藏项',
        'browser.aria': '目录列表',
        'nav.overview': '概览',
        'nav.scanLocations': '扫描位置',
        'nav.cache': '缓存 Caches',
        'nav.appSupport': '应用支持',
        'nav.home': '家目录',
        'nav.addLocation': '添加扫描位置…',
        'nav.settings': '设置',
        'home.heroTitle': '看懂每个目录，再决定要不要清理',
        'home.heroDesc': '磁盘清理工具只告诉你「哪里占地方」，DirDetective 告诉你「这是谁的、存了什么、删了会怎样」。',
        'home.scanCaches': '扫描缓存目录',
        'home.chooseOther': '选择其他目录…',
        'home.statScanned': '已扫描目录',
        'home.statAnalyzed': '已分析',
        'home.statCleanable': '建议可清理',
        'home.privacy': '仅分析目录名与文件名，不读取文件内容',
        'dir.currentPathAria': '当前路径',
        'col.name': '名称',
        'col.purpose': '用途',
        'col.status': '建议',
        'col.size': '大小',
        'col.actions': '操作',
        'settings.title': '设置',
        'settings.tab.basic': '基本',
        'settings.tab.models': '模型配置',
        'settings.tab.whitelist': '已保护目录',
        'settings.tab.knowledge': '已识别目录',
        'settings.tab.guard': '清理防护',
        'settings.tab.ruleset': '规则库',
        'settings.tab.about': '关于',
        'settings.ruleset.title': '规则库',
        'settings.ruleset.desc': '内置官方规则库，用于快速判定常见目录归属与可删性；可联网检查更新。',
        'settings.back': '← 返回目录',
        'settings.basic.title': '基本',
        'settings.basic.desc': '界面显示与本机配置文件位置。',
        'settings.language': '界面语言',
        'settings.langAuto': '跟随系统',
        'settings.listDisplay': '列表显示',
        'settings.showSize': '显示大小列',
        'settings.showSizeNote': '修改时间默认隐藏，可在目录详情中查看。',
        'settings.concurrency': '分析并发数',
        'settings.conc1': '1（逐个，最稳）',
        'settings.conc4': '4（推荐）',
        'settings.conc8': '8（最快，注意限流）',
        'settings.concNote': '批量「分析当前目录」时同时进行的数量。越大越快，但更易触发厂家限流；设为 1 即逐个分析。',
        'settings.configDir': '配置目录',
        'settings.openConfigDir': '↗ 打开配置目录',
        'settings.configDirNote': 'API Key、已识别目录、自定义扫描位置等都保存在此目录下。',
        'settings.models.title': '模型配置',
        'settings.models.desc': '配置分析服务、API 地址与默认模型。',
        'settings.save': '保存',
        'settings.provider': '模型厂家',
        'settings.provider.custom': '自定义 OpenAI 兼容服务',
        'settings.apiKeyPlaceholder': '留空则保留配置文件中的 Key',
        'settings.model': '模型',
        'settings.fetchModels': '刷新模型列表',
        'settings.whitelist.title': '已保护目录',
        'settings.whitelist.desc': '管理你手动保护、需要避免误删的具体位置。',
        'settings.refresh': '刷新',
        'settings.knowledge.title': '已识别目录',
        'settings.knowledge.desc': '管理已经确认准确、下次可直接复用的分析结果。',
        'settings.guard.title': '清理防护',
        'settings.guard.desc': '为避免误删，以下位置不允许移到废纸篓。此规则内置、不可修改。',
        'about.title': '关于 DirDetective',
        'about.desc': '应用信息与系统详情。',
        'about.appInfo': '应用信息',
        'about.version': '当前版本',
        'about.loading': '正在加载…',
        'about.buildType': '构建类型',
        'about.platform': '系统平台',
        'about.osVersion': '系统版本',
        'about.updateCheck': '更新检查',
        'about.latestVersion': '最新版本',
        'about.notChecked': '未检查',
        'about.updateStatus': '更新状态',
        'about.checkUpdate': '检查更新',
        'about.download': '下载新版本',
        'about.links': '相关链接',
        'about.repo': 'GitHub 仓库',
        'about.issues': '报告问题',
        'status.ready': '准备就绪',
        'detail.title': '目录详情',
        'detail.close': '关闭',
    },
    en: {
        'toolbar.up': 'Go to parent folder',
        'toolbar.scan': 'Scan',
        'toolbar.chooseDir': 'Choose Folder',
        'toolbar.analyzeAll': 'Analyze This Folder',
        'toolbar.preview': 'Preview Payload',
        'toolbar.confirmAll': 'Confirm AI Results',
        'filter.aria': 'Filter directories',
        'filter.all': 'All directories',
        'filter.actionable': 'Actionable',
        'filter.unknown': 'Unanalyzed',
        'filter.hideWhitelist': 'Hide protected',
        'filter.whitelist': 'Protected only',
        'filter.hideHidden': 'Hide hidden',
        'filter.hidden': 'Hidden only',
        'browser.aria': 'Directory list',
        'nav.overview': 'Overview',
        'nav.scanLocations': 'Scan Locations',
        'nav.cache': 'Caches',
        'nav.appSupport': 'Application Support',
        'nav.home': 'Home',
        'nav.addLocation': 'Add Location…',
        'nav.settings': 'Settings',
        'home.heroTitle': 'Understand every folder before you clean',
        'home.heroDesc': 'Disk cleaners only tell you what takes up space. DirDetective tells you whose it is, what it holds, and what happens if you delete it.',
        'home.scanCaches': 'Scan Caches',
        'home.chooseOther': 'Choose Another Folder…',
        'home.statScanned': 'Scanned',
        'home.statAnalyzed': 'Analyzed',
        'home.statCleanable': 'Cleanable',
        'home.privacy': 'Only folder and file names are analyzed — contents are never read',
        'dir.currentPathAria': 'Current path',
        'col.name': 'Name',
        'col.purpose': 'Purpose',
        'col.status': 'Advice',
        'col.size': 'Size',
        'col.actions': 'Actions',
        'settings.title': 'Settings',
        'settings.tab.basic': 'General',
        'settings.tab.models': 'Model',
        'settings.tab.whitelist': 'Protected',
        'settings.tab.knowledge': 'Confirmed',
        'settings.tab.guard': 'Cleanup Guard',
        'settings.tab.ruleset': 'Rule Library',
        'settings.tab.about': 'About',
        'settings.ruleset.title': 'Rule Library',
        'settings.ruleset.desc': 'Built-in official rules for quickly judging common directories; can check for updates online.',
        'settings.back': '← Back',
        'settings.basic.title': 'General',
        'settings.basic.desc': 'Interface display and local config location.',
        'settings.language': 'Language',
        'settings.langAuto': 'Follow system',
        'settings.listDisplay': 'List Display',
        'settings.showSize': 'Show size column',
        'settings.showSizeNote': 'Modified time is hidden by default; view it in folder details.',
        'settings.concurrency': 'Analysis Concurrency',
        'settings.conc1': '1 (sequential, safest)',
        'settings.conc4': '4 (recommended)',
        'settings.conc8': '8 (fastest, watch rate limits)',
        'settings.concNote': 'How many folders to analyze at once during batch analysis. Higher is faster but more likely to hit provider rate limits; set to 1 for sequential.',
        'settings.configDir': 'Config Folder',
        'settings.openConfigDir': '↗ Open Config Folder',
        'settings.configDirNote': 'API keys, confirmed analyses and custom scan locations are all stored here.',
        'settings.models.title': 'Model',
        'settings.models.desc': 'Configure the analysis service, API endpoint and default model.',
        'settings.save': 'Save',
        'settings.provider': 'Provider',
        'settings.provider.custom': 'Custom (OpenAI-compatible)',
        'settings.apiKeyPlaceholder': 'Leave empty to keep the saved key',
        'settings.model': 'Model',
        'settings.fetchModels': 'Refresh Models',
        'settings.whitelist.title': 'Protected Folders',
        'settings.whitelist.desc': 'Manage specific locations you have protected from accidental deletion.',
        'settings.refresh': 'Refresh',
        'settings.knowledge.title': 'Confirmed Analyses',
        'settings.knowledge.desc': 'Manage analyses you have confirmed as accurate for direct reuse next time.',
        'settings.guard.title': 'Cleanup Guard',
        'settings.guard.desc': 'To prevent mistakes, the locations below cannot be moved to Trash. These rules are built in and cannot be changed.',
        'about.title': 'About DirDetective',
        'about.desc': 'App information and system details.',
        'about.appInfo': 'App Info',
        'about.version': 'Version',
        'about.loading': 'Loading…',
        'about.buildType': 'Build',
        'about.platform': 'Platform',
        'about.osVersion': 'OS Version',
        'about.updateCheck': 'Updates',
        'about.latestVersion': 'Latest',
        'about.notChecked': 'Not checked',
        'about.updateStatus': 'Status',
        'about.checkUpdate': 'Check for Updates',
        'about.download': 'Download Update',
        'about.links': 'Links',
        'about.repo': 'GitHub Repo',
        'about.issues': 'Report an Issue',
        'status.ready': 'Ready',
        'detail.title': 'Details',
        'detail.close': 'Close',
    },
};

let currentLang = 'zh';

export function detectSystemLang() {
    const nav = (navigator.language || 'en').toLowerCase();
    return nav.startsWith('zh') ? 'zh' : 'en';
}

// 语言偏好：'auto' | 'zh' | 'en'（存 localStorage）。
export function getLangPref() {
    return localStorage.getItem('langPref') || 'auto';
}

export function setLangPref(pref) {
    localStorage.setItem('langPref', pref);
    currentLang = pref === 'auto' ? detectSystemLang() : pref;
    return currentLang;
}

export function initLang() {
    const pref = getLangPref();
    currentLang = pref === 'auto' ? detectSystemLang() : pref;
    return currentLang;
}

export function getLang() {
    return currentLang;
}

export function t(key, params) {
    const table = translations[currentLang] || translations.zh;
    let str = table[key];
    if (str === undefined) str = translations.zh[key];
    if (str === undefined) str = key;
    if (params) {
        str = str.replace(/\{(\w+)\}/g, (m, name) => (params[name] !== undefined ? params[name] : m));
    }
    return str;
}

// 遍历带 data-i18n / data-i18n-title / data-i18n-placeholder / data-i18n-aria 的元素并填充文案。
export function applyStaticI18n(root = document) {
    root.querySelectorAll('[data-i18n]').forEach((el) => {
        el.textContent = t(el.dataset.i18n);
    });
    root.querySelectorAll('[data-i18n-title]').forEach((el) => {
        el.title = t(el.dataset.i18nTitle);
    });
    root.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
        el.placeholder = t(el.dataset.i18nPlaceholder);
    });
    root.querySelectorAll('[data-i18n-aria]').forEach((el) => {
        el.setAttribute('aria-label', t(el.dataset.i18nAria));
    });
    document.documentElement.lang = currentLang === 'zh' ? 'zh-CN' : 'en';
}
