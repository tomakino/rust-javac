'use strict';

const fs = require('fs');
const path = require('path');
const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let output;
let client;
let startPromise = null;
let tempServerPath = null;
let restarting = false;

function log(message) {
  if (!output) return;
  output.appendLine('[' + new Date().toISOString() + '] ' + message);
}

function configuration() {
  return vscode.workspace.getConfiguration('ravacLsp');
}

function workspaceFolder() {
  const folders = vscode.workspace.workspaceFolders;
  return folders && folders.length > 0 ? folders[0] : undefined;
}

function expandVariables(value, folder) {
  if (typeof value !== 'string') return value;
  return value
    .replace(/\$\{workspaceFolder\}/g, folder.uri.fsPath)
    .replace(/\$\{userHome\}/g, process.env.USERPROFILE || process.env.HOME || '')
    .replace(/\$\{env:([^}]+)\}/g, (_, name) => process.env[name] || '');
}

function resolvePath(raw, folder) {
  const expanded = expandVariables(raw || '', folder);
  if (!expanded) return '';
  return path.isAbsolute(expanded) ? expanded : path.resolve(folder.uri.fsPath, expanded);
}

function prepareServer(serverPath) {
  if (!configuration().get('copyServer', true)) return serverPath;
  try {
    const extension = path.extname(serverPath) || '.exe';
    const destination = path.join(
      path.dirname(serverPath),
      '.ravac-lsp-live-' + process.pid + '-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 8) + extension
    );
    fs.copyFileSync(serverPath, destination);
    tempServerPath = destination;
    log('copied server to ' + destination);
    return destination;
  } catch (error) {
    log('copy server failed (' + error.message + '); using original');
    return serverPath;
  }
}

function removeTempServer() {
  const target = tempServerPath;
  tempServerPath = null;
  if (!target) return;
  try {
    fs.unlinkSync(target);
  } catch (error) {
    setTimeout(() => {
      try { fs.unlinkSync(target); } catch (_) { /* the server still owns it */ }
    }, 1000);
  }
}

// The server is the only completion filter and ranker. vscode-languageclient's
// CompletionItemFeature already registers the provider; this middleware only
// overrides the two fields VS Code would otherwise use to re-filter/re-sort:
// filterText = current prefix, sortText = server order.
function completionMiddleware() {
  return {
    async provideCompletionItem(document, position, context, token, next) {
      const result = await next(document, position, context, token);
      if (!result) return result;
      const list = Array.isArray(result) ? new vscode.CompletionList(result, true) : result;
      const wordRange = document.getWordRangeAtPosition(position, /[A-Za-z_$][A-Za-z0-9_$]*/);
      const prefix = wordRange ? document.getText(wordRange) : '';
      list.items.forEach((item, index) => {
        item.filterText = prefix;
        item.sortText = String(index).padStart(6, '0');
      });
      return new vscode.CompletionList(list.items, true);
    }
  };
}

async function startClient() {
  if (client) return client;
  const folder = workspaceFolder();
  if (!folder) {
    log('no workspace folder; not starting');
    return undefined;
  }
  const settings = configuration();
  const serverPath = resolvePath(settings.get('serverPath', '${workspaceFolder}/target/debug/ravac-lsp.exe'), folder);
  if (!fs.existsSync(serverPath)) {
    const message = '找不到 ravac-lsp: ' + serverPath;
    log(message);
    vscode.window.showErrorMessage(message);
    return undefined;
  }

  const args = [];
  const classPath = expandVariables(settings.get('classPath', ''), folder).trim();
  if (classPath) args.push('--class-path', classPath);
  const sourceLevel = String(settings.get('sourceLevel', '17')).trim();
  if (sourceLevel) args.push('--source', sourceLevel);
  const projectPath = expandVariables(settings.get('projectPath', ''), folder).trim();
  if (projectPath) args.push('--project', projectPath);

  const launchPath = prepareServer(serverPath);
  log('starting: ' + launchPath + ' ' + args.join(' '));
  const serverOptions = {
    command: launchPath,
    args,
    transport: TransportKind.stdio,
    options: { cwd: folder.uri.fsPath, env: process.env }
  };
  const clientOptions = {
    documentSelector: [{ scheme: 'file', language: 'java' }],
    workspaceFolder: folder,
    outputChannel: output,
    traceOutputChannel: output,
    middleware: completionMiddleware()
  };
  const instance = new LanguageClient('ravacLsp', 'Ravac LSP', serverOptions, clientOptions);
  try {
    await instance.start();
    client = instance;
    log('client started');
    return client;
  } catch (error) {
    const detail = error && error.stack ? error.stack : String(error);
    log('start failed: ' + detail);
    vscode.window.showErrorMessage('Ravac LSP 启动失败: ' + (error && error.message ? error.message : String(error)));
    try { await instance.stop(); } catch (_) { /* already stopped */ }
    removeTempServer();
    return undefined;
  }
}

async function ensureClient() {
  if (client) return client;
  if (!startPromise) {
    startPromise = startClient().finally(() => { startPromise = null; });
  }
  return startPromise;
}

async function stopClient() {
  const instance = client;
  client = undefined;
  if (!instance) {
    removeTempServer();
    return;
  }
  log('stopping client');
  try {
    await instance.stop();
  } catch (error) {
    log('stop failed: ' + error.message);
  }
  removeTempServer();
}

async function restartClient() {
  if (restarting) return;
  restarting = true;
  try {
    if (startPromise) {
      try { await startPromise; } catch (_) { /* failed start is handled by the caller */ }
    }
    await stopClient();
    await ensureClient();
  } finally {
    restarting = false;
  }
}

function registerCommands(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('ravacLsp.restart', () => restartClient().catch((error) => log('restart failed: ' + error.message))),
    vscode.commands.registerCommand('ravacLsp.showOutput', () => output.show(true)),
    vscode.commands.registerCommand('ravacLsp.probeCompletion', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;
      try {
        const result = await vscode.commands.executeCommand(
          'vscode.executeCompletionItemProvider',
          editor.document.uri,
          editor.selection.active
        );
        const items = Array.isArray(result) ? result : (result && result.items) || [];
        log('probe: ' + items.length + ' items');
        for (const item of items.slice(0, 40)) {
          log('  ' + (item.sortText || '') + '  ' + item.label);
        }
        const folder = workspaceFolder();
        if (folder) {
          const probeFile = path.join(folder.uri.fsPath, 'target', 'tmp', 'vscode-probe.json');
          fs.mkdirSync(path.dirname(probeFile), { recursive: true });
          const payload = items.map((item) => ({
            label: item.label,
            detail: item.detail,
            sortText: item.sortText,
            filterText: item.filterText,
            insertText: item.insertText && item.insertText.value !== undefined ? item.insertText.value : item.insertText,
            range: item.range,
            additionalTextEdits: item.additionalTextEdits
          }));
          fs.writeFileSync(probeFile, JSON.stringify(payload, null, 2));
          log('probe written: ' + probeFile);
        }
        output.show(true);
      } catch (error) {
        log('probe failed: ' + error.message);
        output.show(true);
      }
    })
  );
}

function activate(context) {
  output = vscode.window.createOutputChannel('Ravac LSP');
  context.subscriptions.push(output);
  registerCommands(context);
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((event) => {
    if (event.affectsConfiguration('ravacLsp')) {
      log('configuration changed');
      restartClient().catch((error) => log('restart failed: ' + error.message));
    }
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(() => {
    restartClient().catch((error) => log('restart failed: ' + error.message));
  }));
  ensureClient().catch((error) => log('start failed: ' + (error.stack || error)));
  log('extension activated');
}

async function deactivate() {
  if (startPromise) {
    try { await startPromise; } catch (_) { /* nothing left to stop */ }
  }
  await stopClient();
}

module.exports = { activate, deactivate };
