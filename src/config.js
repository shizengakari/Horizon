/**
 * Horizon - 設定管理モジュール
 * アプリケーション設定の読み込み、暗号化保存、永続化を管理します。
 */

const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');

// アプリケーションデータ保存先ディレクトリ（AppData/Roaming/Horizon）
// 旧ディレクトリ（AppData/Roaming/Canvas Horizon）が存在する場合は自動移行
function resolveUserDataDir() {
  const roaming = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
  const dir = path.join(roaming, 'Horizon');
  const legacyDir = path.join(roaming, 'Canvas Horizon');

  if (!fs.existsSync(dir)) {
    try {
      fs.mkdirSync(dir, { recursive: true });
    } catch (e) {
      console.error('設定フォルダの作成に失敗しました:', e.message);
    }
  }

  if (fs.existsSync(legacyDir)) {
    for (const fileName of ['config.json', 'manual_completed.json', 'cache.json', '.secret.key']) {
      const newFile = path.join(dir, fileName);
      const oldFile = path.join(legacyDir, fileName);
      if (!fs.existsSync(newFile) && fs.existsSync(oldFile)) {
        try {
          fs.copyFileSync(oldFile, newFile);
        } catch (_) {}
      }
    }
  }

  return dir;
}

const userDir = resolveUserDataDir();
const CONFIG_PATH = path.join(userDir, 'config.json');
const FALLBACK_CONFIG_PATH = path.join(__dirname, '..', 'config.json');
const SECRET_KEY_PATH = path.join(userDir, '.secret.key');

// ============================================================================
// ローカル認証付き暗号化 (enc:v1:<nonce_hex>:<ct_hex>:<tag_hex>) - Rust完全互換
// ============================================================================

function getOrCreateMasterSecret() {
  try {
    if (fs.existsSync(SECRET_KEY_PATH)) {
      const existing = fs.readFileSync(SECRET_KEY_PATH, 'utf-8').trim();
      if (existing.length >= 32) {
        return existing;
      }
    }
  } catch (_) {}

  const secretHex = crypto.randomBytes(32).toString('hex');
  try {
    fs.writeFileSync(SECRET_KEY_PATH, secretHex, 'utf-8');
  } catch (_) {}
  return secretHex;
}

function deriveCipherKeys() {
  const secret = getOrCreateMasterSecret();
  const computer = String(process.env.COMPUTERNAME || process.env.HOSTNAME || 'local-pc').trim().toLowerCase();
  const user = String(process.env.USERNAME || process.env.USER || 'local-user').trim().toLowerCase();
  const encKey = crypto.createHash('sha256').update(`horizon-enc-v1:${secret}:${computer}:${user}`, 'utf8').digest();
  const macKey = crypto.createHash('sha256').update(`horizon-mac-v1:${secret}:${computer}:${user}`, 'utf8').digest();
  return { encKey, macKey };
}

function isEncryptedSecret(val) {
  return typeof val === 'string' && val.trim().startsWith('enc:v1:');
}

function encryptSecret(plaintext) {
  if (typeof plaintext !== 'string') return '';
  const trimmed = plaintext.trim();
  if (!trimmed) return '';
  if (isEncryptedSecret(trimmed)) return trimmed;

  const { encKey, macKey } = deriveCipherKeys();
  const nonce = crypto.randomBytes(16);
  const ptBuf = Buffer.from(trimmed, 'utf8');
  const ctBuf = Buffer.alloc(ptBuf.length);

  const totalChunks = Math.ceil(ptBuf.length / 32);
  for (let chunkIdx = 0; chunkIdx < totalChunks; chunkIdx++) {
    const blockIn = Buffer.alloc(52);
    encKey.copy(blockIn, 0);
    nonce.copy(blockIn, 32);
    blockIn.writeUInt32BE(chunkIdx >>> 0, 48);
    const streamBlock = crypto.createHash('sha256').update(blockIn).digest();

    const offset = chunkIdx * 32;
    const len = Math.min(32, ptBuf.length - offset);
    for (let i = 0; i < len; i++) {
      ctBuf[offset + i] = ptBuf[offset + i] ^ streamBlock[i];
    }
  }

  const macInput = Buffer.concat([nonce, ctBuf]);
  const tagFull = crypto.createHmac('sha256', macKey).update(macInput).digest();
  const tag = tagFull.subarray(0, 16);

  return `enc:v1:${nonce.toString('hex')}:${ctBuf.toString('hex')}:${tag.toString('hex')}`;
}

function decryptSecret(value) {
  if (typeof value !== 'string') return '';
  const trimmed = value.trim();
  if (!trimmed) return '';
  if (!isEncryptedSecret(trimmed)) return trimmed;

  const parts = trimmed.split(':');
  if (parts.length !== 5 || parts[0] !== 'enc' || parts[1] !== 'v1') {
    return '';
  }
  try {
    const nonce = Buffer.from(parts[2], 'hex');
    const ctBuf = Buffer.from(parts[3], 'hex');
    const tag = Buffer.from(parts[4], 'hex');
    if (nonce.length !== 16 || tag.length !== 16) return '';

    const { encKey, macKey } = deriveCipherKeys();
    const macInput = Buffer.concat([nonce, ctBuf]);
    const expectedTag = crypto.createHmac('sha256', macKey).update(macInput).digest().subarray(0, 16);
    if (!crypto.timingSafeEqual(tag, expectedTag)) {
      return '';
    }

    const ptBuf = Buffer.alloc(ctBuf.length);
    const totalChunks = Math.ceil(ctBuf.length / 32);
    for (let chunkIdx = 0; chunkIdx < totalChunks; chunkIdx++) {
      const blockIn = Buffer.alloc(52);
      encKey.copy(blockIn, 0);
      nonce.copy(blockIn, 32);
      blockIn.writeUInt32BE(chunkIdx >>> 0, 48);
      const streamBlock = crypto.createHash('sha256').update(blockIn).digest();

      const offset = chunkIdx * 32;
      const len = Math.min(32, ctBuf.length - offset);
      for (let i = 0; i < len; i++) {
        ptBuf[offset + i] = ctBuf[offset + i] ^ streamBlock[i];
      }
    }

    return ptBuf.toString('utf8');
  } catch (_) {
    return '';
  }
}

function maskToken(token) {
  if (typeof token !== 'string') return '';
  const trimmed = token.trim();
  if (!trimmed) return '';
  const chars = Array.from(trimmed);
  if (chars.length <= 8) return '••••••••';
  return `${chars.slice(0, 4).join('')}••••••••${chars.slice(-4).join('')}`;
}

// メモリキャッシュ（API呼び出しごとの同期ディスクI/OとJSONパースを回避）
let cachedConfig = null;

/**
 * 現在の設定を読み込みます（メモリ上では復号済み）。
 * @param {boolean} [forceReload=false] キャッシュを無視してディスクから再読み込みするか
 * @returns {object} アプリケーション設定オブジェクト
 */
function loadConfig(forceReload = false) {
  if (cachedConfig && !forceReload) {
    return { ...cachedConfig };
  }

  let config = {
    baseUrl: '',
    apiToken: '',
    theme: 'dark',
    currentQuarter: '',
    batteryMode: 'auto', // 'auto' (バッテリー駆動時自動) | 'on' (常時有効) | 'off' (無効)
    batterySaver: false,
    pollIntervalMin: 15,
    autoDownloadFolder: '',
    githubRepo: 'shizengakari/Horizon'
  };

  let needsEncryptionUpgrade = false;

  // 1. AppData からユーザー設定を読み込み
  if (fs.existsSync(CONFIG_PATH)) {
    try {
      const raw = fs.readFileSync(CONFIG_PATH, 'utf-8').replace(/^\uFEFF/, '').trim();
      const data = JSON.parse(raw);
      if (
        (data.apiToken && !isEncryptedSecret(data.apiToken)) ||
        (data.baseUrl && !isEncryptedSecret(data.baseUrl))
      ) {
        needsEncryptionUpgrade = true;
      }
      data.baseUrl = decryptSecret(data.baseUrl || '');
      data.apiToken = decryptSecret(data.apiToken || '');
      config = { ...config, ...data };
    } catch (e) {
      console.error('AppData からの設定読み込みエラー:', e.message);
    }
  } else if (fs.existsSync(FALLBACK_CONFIG_PATH)) {
    // 2. ローカルディレクトリのフォールバック設定を読み込み
    try {
      const raw = fs.readFileSync(FALLBACK_CONFIG_PATH, 'utf-8').replace(/^\uFEFF/, '').trim();
      const data = JSON.parse(raw);
      data.baseUrl = decryptSecret(data.baseUrl || '');
      data.apiToken = decryptSecret(data.apiToken || '');
      config = { ...config, ...data };
      if (config.baseUrl || config.apiToken) {
        needsEncryptionUpgrade = true;
      }
    } catch (e) {
      console.error('フォールバック設定の読み込みエラー:', e.message);
    }
  }

  // batteryMode と batterySaver の相互互換性
  if (!config.batteryMode || !['auto', 'on', 'off'].includes(config.batteryMode)) {
    if (config.batterySaver !== undefined) {
      config.batteryMode = config.batterySaver ? 'on' : 'auto';
    } else {
      config.batteryMode = 'auto';
    }
  }
  config.batterySaver = config.batteryMode === 'on';

  // githubRepo の正規化 (旧リポジトリ名や未設定の場合は公式リポジトリを適用)
  const OFFICIAL_REPO = 'shizengakari/Horizon';
  if (
    !config.githubRepo ||
    config.githubRepo === 'shize-tech/CanvasLMS' ||
    config.githubRepo === 'shizengakari/CanvasLMS-Horizon'
  ) {
    config.githubRepo = OFFICIAL_REPO;
  }

  // 3. 環境変数（CI/CD またはローカル開発用）からの補完
  if (!config.baseUrl && process.env.CANVAS_BASE_URL) {
    config.baseUrl = process.env.CANVAS_BASE_URL;
  }
  if (!config.apiToken && process.env.CANVAS_API_TOKEN) {
    config.apiToken = process.env.CANVAS_API_TOKEN;
  }

  cachedConfig = config;

  // 平文で保存されていた設定ファイルがあれば自動的に暗号化形式へアップグレード保存
  if (needsEncryptionUpgrade) {
    try {
      const diskCopy = {
        ...config,
        baseUrl: encryptSecret(config.baseUrl),
        apiToken: encryptSecret(config.apiToken)
      };
      fs.writeFileSync(CONFIG_PATH, JSON.stringify(diskCopy, null, 2), 'utf-8');
    } catch (_) {}
  }

  return { ...cachedConfig };
}

/**
 * 更新された設定を暗号化して保存します。
 * @param {object} newConfig 保存する新しい設定情報
 * @returns {boolean} 成功可否
 */
function saveConfig(newConfig) {
  try {
    const current = loadConfig();
    const patch = { ...newConfig };

    // マスクされたトークン（••••）や空文字が送信された場合は既存のトークンを維持
    if (patch.apiToken !== undefined) {
      const trimmedToken = String(patch.apiToken).trim();
      if (!trimmedToken || trimmedToken.includes('•')) {
        delete patch.apiToken;
      } else {
        patch.apiToken = decryptSecret(trimmedToken);
      }
    }
    if (patch.baseUrl !== undefined) {
      const trimmedUrl = String(patch.baseUrl).trim().replace(/\/+$/, '');
      if (trimmedUrl) {
        patch.baseUrl = decryptSecret(trimmedUrl);
      } else {
        delete patch.baseUrl;
      }
    }

    const merged = { ...current, ...patch };
    if (newConfig.batteryMode !== undefined) {
      if (['auto', 'on', 'off'].includes(newConfig.batteryMode)) {
        merged.batteryMode = newConfig.batteryMode;
      } else {
        merged.batteryMode = 'auto';
      }
      merged.batterySaver = merged.batteryMode === 'on';
    } else if (newConfig.batterySaver !== undefined) {
      merged.batterySaver = Boolean(newConfig.batterySaver);
      merged.batteryMode = merged.batterySaver ? 'on' : 'auto';
    }

    const diskCopy = {
      ...merged,
      baseUrl: encryptSecret(merged.baseUrl),
      apiToken: encryptSecret(merged.apiToken)
    };
    fs.writeFileSync(CONFIG_PATH, JSON.stringify(diskCopy, null, 2), 'utf-8');
    cachedConfig = { ...merged };
    return true;
  } catch (err) {
    console.error('設定の保存に失敗しました:', err.message);
    return false;
  }
}

module.exports = {
  loadConfig,
  saveConfig,
  maskToken,
  encryptSecret,
  decryptSecret,
  isEncryptedSecret,
  getUserDataDir: () => userDir
};
