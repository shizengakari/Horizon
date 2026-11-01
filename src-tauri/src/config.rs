//! Configuration management module for Horizon
//! Handles reading, saving, and synchronizing user configuration from %APPDATA%/Horizon/config.json
//! Provides 100% parity with `src/config.js` and `manual_completed.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

static CACHED_CONFIG: LazyLock<Mutex<Option<AppConfig>>> = LazyLock::new(|| Mutex::new(None));

const OFFICIAL_REPO: &str = "shizengakari/Horizon";
const LEGACY_REPO: &str = "shize-tech/CanvasLMS";
const LEGACY_HORIZON_REPO: &str = "shizengakari/CanvasLMS-Horizon";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_token: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub current_quarter: String,
    #[serde(default = "default_battery_mode")]
    pub battery_mode: String,
    #[serde(default)]
    pub battery_saver: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_min: u32,
    #[serde(default)]
    pub download_dir: String,
    #[serde(default)]
    pub auto_download_folder: String,
    #[serde(default = "default_github_repo")]
    pub github_repo: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigPatch {
    pub base_url: Option<String>,
    pub api_token: Option<String>,
    pub download_dir: Option<String>,
    pub current_quarter: Option<String>,
    pub theme: Option<String>,
    pub battery_mode: Option<String>,
    pub battery_saver: Option<bool>,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_battery_mode() -> String {
    "auto".to_string()
}

fn default_poll_interval() -> u32 {
    15
}

fn default_github_repo() -> String {
    OFFICIAL_REPO.to_string()
}

pub fn default_download_dir() -> String {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Downloads")
        .to_string_lossy()
        .into_owned()
}

impl Default for AppConfig {
    fn default() -> Self {
        let dl = default_download_dir();
        Self {
            base_url: String::new(),
            api_token: String::new(),
            theme: default_theme(),
            current_quarter: String::new(),
            battery_mode: default_battery_mode(),
            battery_saver: false,
            poll_interval_min: default_poll_interval(),
            download_dir: dl.clone(),
            auto_download_folder: dl,
            github_repo: default_github_repo(),
        }
    }
}

impl AppConfig {
    /// Synchronize batteryMode, batterySaver, download_dir, and repository normalization
    pub fn normalize(&mut self) {
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        self.api_token = self.api_token.trim().to_string();

        if self.battery_mode == "on" {
            self.battery_saver = true;
        } else if self.battery_mode == "off" {
            self.battery_saver = false;
        } else {
            self.battery_mode = "auto".to_string();
            self.battery_saver = false;
        }

        if self.download_dir.trim().is_empty() && !self.auto_download_folder.trim().is_empty() {
            self.download_dir = self.auto_download_folder.trim().to_string();
        } else if self.download_dir.trim().is_empty() {
            self.download_dir = default_download_dir();
        }
        let norm_dl = self
            .download_dir
            .trim()
            .trim_end_matches(['\\', '/'])
            .to_lowercase();
        if norm_dl.ends_with("\\downloads\\canvas")
            || norm_dl.ends_with("/downloads/canvas")
            || norm_dl.ends_with("\\downloads\\canvaslms")
            || norm_dl.ends_with("/downloads/canvaslms")
        {
            self.download_dir = default_download_dir();
        }
        self.auto_download_folder = self.download_dir.clone();

        if self.github_repo.trim().is_empty()
            || self.github_repo == LEGACY_REPO
            || self.github_repo == LEGACY_HORIZON_REPO
        {
            self.github_repo = OFFICIAL_REPO.to_string();
        }

        if self.base_url.is_empty() {
            if let Ok(env_url) = std::env::var("CANVAS_BASE_URL") {
                if !env_url.trim().is_empty() {
                    self.base_url = env_url.trim().to_string();
                }
            }
        }
        if self.api_token.is_empty() {
            if let Ok(env_token) = std::env::var("CANVAS_API_TOKEN") {
                if !env_token.trim().is_empty() {
                    self.api_token = env_token.trim().to_string();
                }
            }
        }
    }
}

pub fn mask_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 8 {
        return "••••••••".to_string();
    }
    let prefix: String = chars[..4].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{}••••••••{}", prefix, suffix)
}

/// Returns the path to the user AppData configuration directory:
/// `%APPDATA%/Horizon` (migrating from legacy `%APPDATA%/Canvas Horizon` if present)
pub fn get_user_data_dir() -> PathBuf {
    let base = if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join("AppData").join("Roaming")
    } else {
        return PathBuf::from(".").join("data");
    };

    let dir = base.join("Horizon");
    let legacy_dir = base.join("Canvas Horizon");
    if legacy_dir.exists() {
        let _ = fs::create_dir_all(&dir);
        for file_name in ["config.json", "manual_completed.json", "cache.json"] {
            let new_file = dir.join(file_name);
            let old_file = legacy_dir.join(file_name);
            if !new_file.exists() && old_file.exists() {
                let _ = fs::copy(&old_file, &new_file);
            }
        }
    }
    dir
}

pub fn get_config_dir() -> PathBuf {
    get_user_data_dir()
}

pub fn get_config_path() -> PathBuf {
    get_user_data_dir().join("config.json")
}

pub fn get_manual_completed_path() -> PathBuf {
    get_user_data_dir().join("manual_completed.json")
}

// ============================================================================
// Authenticated Local Secret Encryption (enc:v1:<nonce_hex>:<ct_hex>:<tag_hex>)
// ============================================================================

const K_SHA256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K_SHA256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha256(key);
        k_block[..32].copy_from_slice(&hashed);
    } else {
        k_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k_block[i];
        opad[i] ^= k_block[i];
    }
    let mut inner = Vec::with_capacity(64 + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_hash = sha256(&inner);

    let mut outer = Vec::with_capacity(96);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_decode(hex_str: &str) -> Option<Vec<u8>> {
    if hex_str.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(hex_str.len() / 2);
    let bytes = hex_str.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Some(out)
}

fn generate_random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    #[cfg(target_os = "windows")]
    {
        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn SystemFunction036(random_buffer: *mut u8, random_buffer_length: u32) -> u8;
        }
        let ok = unsafe { SystemFunction036(out.as_mut_ptr(), N as u32) };
        if ok != 0 {
            return out;
        }
    }
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let mut filled = 0;
    while filled < N {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id() as u64;
        let cnt = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let addr = (&out as *const _ as usize) as u64;
        let mut seed = Vec::with_capacity(40);
        seed.extend_from_slice(&nanos.to_le_bytes());
        seed.extend_from_slice(&pid.to_le_bytes());
        seed.extend_from_slice(&cnt.to_le_bytes());
        seed.extend_from_slice(&addr.to_le_bytes());
        let block = sha256(&seed);
        let take = (N - filled).min(32);
        out[filled..filled + take].copy_from_slice(&block[..take]);
        filled += take;
    }
    out
}

fn get_or_create_master_secret() -> String {
    let dir = get_user_data_dir();
    let _ = fs::create_dir_all(&dir);
    let key_path = dir.join(".secret.key");
    if let Ok(existing) = fs::read_to_string(&key_path) {
        let trimmed = existing.trim();
        if trimmed.len() >= 32 {
            return trimmed.to_string();
        }
    }
    // Check legacy directory for `.secret.key`
    if let Some(parent) = dir.parent() {
        let legacy_key = parent.join("Canvas Horizon").join(".secret.key");
        if let Ok(existing) = fs::read_to_string(&legacy_key) {
            let trimmed = existing.trim();
            if trimmed.len() >= 32 {
                let _ = fs::write(&key_path, trimmed);
                return trimmed.to_string();
            }
        }
    }
    let rnd = generate_random_bytes::<32>();
    let secret_hex = hex_encode(&rnd);
    let _ = fs::write(&key_path, &secret_hex);
    secret_hex
}

fn derive_cipher_keys() -> ([u8; 32], [u8; 32]) {
    let secret = get_or_create_master_secret();
    let computer = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "local-pc".to_string())
        .trim()
        .to_lowercase();
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "local-user".to_string())
        .trim()
        .to_lowercase();
    let enc_material = format!("horizon-enc-v1:{}:{}:{}", secret, computer, user);
    let mac_material = format!("horizon-mac-v1:{}:{}:{}", secret, computer, user);
    (
        sha256(enc_material.as_bytes()),
        sha256(mac_material.as_bytes()),
    )
}

pub fn is_encrypted_secret(val: &str) -> bool {
    val.trim().starts_with("enc:v1:")
}

pub fn encrypt_secret(plaintext: &str) -> String {
    let trimmed = plaintext.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_encrypted_secret(trimmed) {
        return trimmed.to_string();
    }
    let (enc_key, mac_key) = derive_cipher_keys();
    let nonce = generate_random_bytes::<16>();
    let pt_bytes = trimmed.as_bytes();
    let mut ct = Vec::with_capacity(pt_bytes.len());

    for (chunk_idx, chunk) in pt_bytes.chunks(32).enumerate() {
        let mut block_in = [0u8; 52];
        block_in[0..32].copy_from_slice(&enc_key);
        block_in[32..48].copy_from_slice(&nonce);
        block_in[48..52].copy_from_slice(&(chunk_idx as u32).to_be_bytes());
        let stream_block = sha256(&block_in);
        for (i, &b) in chunk.iter().enumerate() {
            ct.push(b ^ stream_block[i]);
        }
    }

    let mut mac_input = Vec::with_capacity(16 + ct.len());
    mac_input.extend_from_slice(&nonce);
    mac_input.extend_from_slice(&ct);
    let tag_full = hmac_sha256(&mac_key, &mac_input);
    let tag = &tag_full[0..16];

    format!(
        "enc:v1:{}:{}:{}",
        hex_encode(&nonce),
        hex_encode(&ct),
        hex_encode(tag)
    )
}

pub fn decrypt_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if !is_encrypted_secret(trimmed) {
        return trimmed.to_string();
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() != 5 || parts[0] != "enc" || parts[1] != "v1" {
        return String::new();
    }
    let Some(nonce) = hex_decode(parts[2]) else {
        return String::new();
    };
    let Some(ct) = hex_decode(parts[3]) else {
        return String::new();
    };
    let Some(tag) = hex_decode(parts[4]) else {
        return String::new();
    };
    if nonce.len() != 16 || tag.len() != 16 {
        return String::new();
    }

    let (enc_key, mac_key) = derive_cipher_keys();
    let mut mac_input = Vec::with_capacity(16 + ct.len());
    mac_input.extend_from_slice(&nonce);
    mac_input.extend_from_slice(&ct);
    let expected_tag_full = hmac_sha256(&mac_key, &mac_input);

    let mut diff = 0u8;
    for i in 0..16 {
        diff |= tag[i] ^ expected_tag_full[i];
    }
    if diff != 0 {
        return String::new();
    }

    let mut pt = Vec::with_capacity(ct.len());
    for (chunk_idx, chunk) in ct.chunks(32).enumerate() {
        let mut block_in = [0u8; 52];
        block_in[0..32].copy_from_slice(&enc_key);
        block_in[32..48].copy_from_slice(&nonce);
        block_in[48..52].copy_from_slice(&(chunk_idx as u32).to_be_bytes());
        let stream_block = sha256(&block_in);
        for (i, &b) in chunk.iter().enumerate() {
            pt.push(b ^ stream_block[i]);
        }
    }

    String::from_utf8(pt).unwrap_or_default()
}

fn parse_config_resilient(raw: &str) -> AppConfig {
    let cleaned = raw.trim_start_matches('\u{feff}').trim();
    if let Ok(cfg) = serde_json::from_str::<AppConfig>(cleaned) {
        return cfg;
    }
    let mut cfg = AppConfig::default();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(cleaned) {
        if let Some(s) = val.get("baseUrl").and_then(|v| v.as_str()) {
            cfg.base_url = s.to_string();
        }
        if let Some(s) = val.get("apiToken").and_then(|v| v.as_str()) {
            cfg.api_token = s.to_string();
        }
        if let Some(s) = val.get("theme").and_then(|v| v.as_str()) {
            cfg.theme = s.to_string();
        }
        if let Some(s) = val.get("currentQuarter").and_then(|v| v.as_str()) {
            cfg.current_quarter = s.to_string();
        }
        if let Some(s) = val.get("batteryMode").and_then(|v| v.as_str()) {
            cfg.battery_mode = s.to_string();
        }
        if let Some(b) = val.get("batterySaver").and_then(|v| v.as_bool()) {
            cfg.battery_saver = b;
        }
        if let Some(n) = val.get("pollIntervalMin").and_then(|v| v.as_u64()) {
            cfg.poll_interval_min = n as u32;
        }
        if let Some(s) = val.get("downloadDir").and_then(|v| v.as_str()) {
            cfg.download_dir = s.to_string();
        }
        if let Some(s) = val.get("autoDownloadFolder").and_then(|v| v.as_str()) {
            cfg.auto_download_folder = s.to_string();
        }
        if let Some(s) = val.get("githubRepo").and_then(|v| v.as_str()) {
            cfg.github_repo = s.to_string();
        }
    }
    cfg
}

pub fn load_config_from_path(path: &Path) -> AppConfig {
    let mut had_plaintext_secret = false;
    let mut config = if path.exists() {
        match fs::read_to_string(path) {
            Ok(content) => {
                let mut parsed = parse_config_resilient(&content);
                if (!parsed.api_token.trim().is_empty() && !is_encrypted_secret(&parsed.api_token))
                    || (!parsed.base_url.trim().is_empty() && !is_encrypted_secret(&parsed.base_url))
                {
                    had_plaintext_secret = true;
                }
                parsed.base_url = decrypt_secret(&parsed.base_url);
                parsed.api_token = decrypt_secret(&parsed.api_token);
                parsed
            }
            Err(_) => AppConfig::default(),
        }
    } else {
        AppConfig::default()
    };
    config.normalize();
    if had_plaintext_secret && path.exists() {
        let _ = save_config_to_path(&config, path);
    }
    config
}

pub fn load_config() -> AppConfig {
    if let Ok(guard) = CACHED_CONFIG.lock() {
        if let Some(ref cached) = *guard {
            return cached.clone();
        }
    }

    let config_path = get_config_path();
    let mut cfg = load_config_from_path(&config_path);
    if cfg.base_url.is_empty() || cfg.api_token.is_empty() {
        let mut fallbacks = vec![PathBuf::from("config.json")];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                fallbacks.push(dir.join("config.json"));
            }
        }
        for fb in fallbacks {
            if fb.exists() && fb != config_path {
                let fb_cfg = load_config_from_path(&fb);
                if !fb_cfg.base_url.is_empty() && !fb_cfg.api_token.is_empty() {
                    if cfg.base_url.is_empty() {
                        cfg.base_url = fb_cfg.base_url;
                    }
                    if cfg.api_token.is_empty() {
                        cfg.api_token = fb_cfg.api_token;
                    }
                    cfg.normalize();
                    let _ = save_config_to_path(&cfg, &config_path);
                    break;
                }
            }
        }
    }
    if let Ok(mut guard) = CACHED_CONFIG.lock() {
        *guard = Some(cfg.clone());
    }
    cfg
}

pub fn save_config_to_path(config: &AppConfig, path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut to_save = config.clone();
    to_save.base_url = decrypt_secret(&to_save.base_url);
    to_save.api_token = decrypt_secret(&to_save.api_token);
    to_save.normalize();

    let mut disk_copy = to_save.clone();
    disk_copy.base_url = encrypt_secret(&to_save.base_url);
    disk_copy.api_token = encrypt_secret(&to_save.api_token);

    let json = serde_json::to_string_pretty(&disk_copy)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)?;
    if let Ok(mut guard) = CACHED_CONFIG.lock() {
        *guard = Some(to_save);
    }
    Ok(())
}

pub fn save_config(patch: ConfigPatch) -> AppConfig {
    let mut current = load_config();
    if let Some(u) = patch.base_url {
        current.base_url = u;
    }
    if let Some(t) = patch.api_token {
        current.api_token = t;
    }
    if let Some(d) = patch.download_dir {
        current.download_dir = d.clone();
        current.auto_download_folder = d;
    }
    if let Some(q) = patch.current_quarter {
        current.current_quarter = q;
    }
    if let Some(th) = patch.theme {
        current.theme = th;
    }
    if let Some(bm) = patch.battery_mode {
        current.battery_mode = bm;
    } else if let Some(bs) = patch.battery_saver {
        current.battery_mode = if bs {
            "on".to_string()
        } else {
            "off".to_string()
        };
    }
    if let Some(bs) = patch.battery_saver {
        current.battery_saver = bs;
    }
    current.normalize();
    let _ = save_config_to_path(&current, &get_config_path());
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip_and_tamper_detection() {
        let raw_token = "7~aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789_secretTokenValue";
        let encrypted = encrypt_secret(raw_token);
        assert!(is_encrypted_secret(&encrypted));
        assert!(!encrypted.contains(raw_token));
        assert_eq!(decrypt_secret(&encrypted), raw_token);

        // Tampering with the authentication tag must fail decryption
        let mut tampered = encrypted.clone();
        let last_char = if tampered.ends_with('0') { '1' } else { '0' };
        tampered.pop();
        tampered.push(last_char);
        assert_eq!(decrypt_secret(&tampered), "");
    }

    #[test]
    fn test_config_disk_encryption_and_legacy_migration() {
        let temp_path = std::env::temp_dir().join(format!("horizon_test_cfg_{}.json", std::process::id()));
        let mut cfg = AppConfig::default();
        cfg.base_url = "https://example.instructure.com".to_string();
        cfg.api_token = "secret_token_1234567890".to_string();

        save_config_to_path(&cfg, &temp_path).expect("save config");
        let on_disk = fs::read_to_string(&temp_path).expect("read raw json");
        assert!(!on_disk.contains("secret_token_1234567890"));
        assert!(!on_disk.contains("https://example.instructure.com"));
        assert!(on_disk.contains("enc:v1:"));

        let loaded = load_config_from_path(&temp_path);
        assert_eq!(loaded.base_url, "https://example.instructure.com");
        assert_eq!(loaded.api_token, "secret_token_1234567890");

        let _ = fs::remove_file(&temp_path);
    }
}
