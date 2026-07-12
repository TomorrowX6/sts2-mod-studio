//! Godot `.pck` 只读解析（格式版本 3，Godot 4.4+ 导出；STS2 用 Godot 4.5.1）。
//!
//! 布局（小端，已对 Godot 4.5.1 真实导出的 pck 逐字节核对）：
//! ```text
//! u32 magic "GDPC" | u32 formatVersion(=3) | u32×3 godot 版本
//! u32 packFlags（1=目录加密 2=REL_FILEBASE）
//! u64 fileBase | u64 dirOffset | 保留区
//! 目录位于 dirOffset：u32 fileCount，然后每文件：
//!   u32 pathLen | path（\0 填充到 pathLen，不带 res:// 前缀）
//!   | u64 offset（相对 fileBase）| u64 size | 16B md5 | u32 fileFlags
//! ```

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{bail, Context, Result};

const MAGIC: u32 = 0x4350_4447; // "GDPC"
const PACK_DIR_ENCRYPTED: u32 = 1;
const FILE_ENCRYPTED: u32 = 1 << 0;
const FILE_REMOVAL: u32 = 1 << 1;

#[derive(Debug, Clone)]
pub struct PckEntry {
    /// 资源路径（已去掉 res:// 前缀），如 `MyMod/localization/zhs/cards.json`。
    pub path: String,
    /// 文件数据在 pck 中的绝对偏移。
    pub offset: u64,
    pub size: u64,
    pub encrypted: bool,
}

pub struct Pck {
    file: File,
    pub entries: Vec<PckEntry>,
}

impl Pck {
    pub fn open(path: &Path) -> Result<Self> {
        let mut f = File::open(path).with_context(|| format!("打开 {} 失败", path.display()))?;
        if read_u32(&mut f)? != MAGIC {
            bail!("{} 不是 Godot pck 文件（magic 不符）", path.display());
        }
        let version = read_u32(&mut f)?;
        if version != 3 {
            bail!(
                "不支持的 pck 格式版本 {version}（仅支持 Godot 4.4+ 的版本 3；\
                 STS2 mod 均由 Godot 4.5 导出）"
            );
        }
        let _godot_major = read_u32(&mut f)?;
        let _godot_minor = read_u32(&mut f)?;
        let _godot_patch = read_u32(&mut f)?;
        let pack_flags = read_u32(&mut f)?;
        if pack_flags & PACK_DIR_ENCRYPTED != 0 {
            bail!("pck 目录已加密，无法读取");
        }
        let file_base = read_u64(&mut f)?;
        let dir_offset = read_u64(&mut f)?;

        f.seek(SeekFrom::Start(dir_offset))?;
        let count = read_u32(&mut f)?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let path_len = read_u32(&mut f)? as usize;
            if path_len > 64 * 1024 {
                bail!("pck 目录损坏：路径长度异常（{path_len}）");
            }
            let mut buf = vec![0u8; path_len];
            f.read_exact(&mut buf)?;
            let rel = String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string();
            let rel = rel.strip_prefix("res://").unwrap_or(&rel).to_string();
            let offset = read_u64(&mut f)?;
            let size = read_u64(&mut f)?;
            f.seek(SeekFrom::Current(16))?; // md5
            let flags = read_u32(&mut f)?;
            if flags & FILE_REMOVAL != 0 {
                continue; // 补丁 pck 的删除标记
            }
            entries.push(PckEntry {
                path: rel,
                offset: file_base + offset,
                size,
                encrypted: flags & FILE_ENCRYPTED != 0,
            });
        }
        Ok(Pck { file: f, entries })
    }

    /// 读取一个条目的完整内容。
    pub fn read(&mut self, entry: &PckEntry) -> Result<Vec<u8>> {
        if entry.encrypted {
            bail!("文件已加密，无法读取: {}", entry.path);
        }
        self.file.seek(SeekFrom::Start(entry.offset))?;
        let mut buf = vec![0u8; entry.size as usize];
        self.file
            .read_exact(&mut buf)
            .with_context(|| format!("读取 {} 失败", entry.path))?;
        Ok(buf)
    }
}

/// 从 CompressedTexture2D（.ctex，GST2 容器）里抠出内嵌的无损图片数据。
/// 卡图等 2D 纹理默认以无损 WebP/PNG 内嵌；VRAM 压缩纹理无法还原，返回 None。
/// 返回 (扩展名, 图片字节)。
pub fn extract_ctex_image(ctex: &[u8]) -> Option<(&'static str, Vec<u8>)> {
    // WebP: RIFF <u32 size> WEBP …，总长 = size + 8
    if let Some(pos) = find(ctex, b"RIFF") {
        if ctex.get(pos + 8..pos + 12) == Some(b"WEBP") {
            let size = u32::from_le_bytes(ctex.get(pos + 4..pos + 8)?.try_into().ok()?) as usize;
            let end = pos + 8 + size;
            if end <= ctex.len() {
                return Some(("webp", ctex[pos..end].to_vec()));
            }
        }
    }
    // PNG: 89 50 4E 47 … IEND + CRC
    if let Some(pos) = find(ctex, b"\x89PNG\r\n\x1a\n") {
        let iend = find(&ctex[pos..], b"IEND")?;
        let end = pos + iend + 8; // IEND 类型 4 字节 + CRC 4 字节
        if end <= ctex.len() {
            return Some(("png", ctex[pos..end].to_vec()));
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn read_u32(f: &mut File) -> Result<u32> {
    let mut b = [0u8; 4];
    f.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(f: &mut File) -> Result<u64> {
    let mut b = [0u8; 8];
    f.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}
