//! 缓存仅在固定入口目录下写入，通过目录 FD 防止符号链接重定向。

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

use nix::fcntl::{OFlag, open, openat, renameat};
use nix::sys::stat::{Mode, mkdirat};
use nix::unistd::{UnlinkatFlags, unlinkat};

fn directory(path: &Path) -> anyhow::Result<File> {
    let flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let mut directory = File::from(open(Path::new("/"), flags, Mode::empty())?);
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                directory = File::from(openat(&directory, name, flags, Mode::empty())?);
            }
            _ => anyhow::bail!("远程规则缓存目录必须为规范绝对路径"),
        }
    }
    Ok(directory)
}

pub(super) fn read(path: &Path, max_bytes: usize) -> anyhow::Result<String> {
    let parent = directory(path.parent().ok_or_else(|| anyhow::anyhow!("缓存路径无效"))?)?;
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("缓存路径无效"))?;
    let file = File::from(openat(
        &parent,
        name,
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?);
    anyhow::ensure!(file.metadata()?.is_file(), "规则缓存不是普通文件");
    let mut content = String::new();
    file.take((max_bytes as u64).saturating_add(1))
        .read_to_string(&mut content)?;
    anyhow::ensure!(content.len() <= max_bytes, "远程规则缓存超过容量限制");
    Ok(content)
}

pub(super) fn replace(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("缓存路径无效"))?;
    let root = directory(
        parent
            .parent()
            .ok_or_else(|| anyhow::anyhow!("缓存路径无效"))?,
    )?;
    let cache_name = parent
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("缓存路径无效"))?;
    match mkdirat(&root, cache_name, Mode::from_bits_truncate(0o700)) {
        Ok(()) | Err(nix::errno::Errno::EEXIST) => {}
        Err(error) => return Err(error.into()),
    }
    let cache = directory(parent)?;
    let temporary = format!(".{}.tmp", uuid::Uuid::new_v4());
    let mut file = File::from(openat(
        &cache,
        temporary.as_str(),
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::from_bits_truncate(0o600),
    )?);
    let result = (|| -> anyhow::Result<()> {
        file.write_all(content)?;
        file.sync_all()?;
        let current = directory(parent)?;
        let before = cache.metadata()?;
        let after = current.metadata()?;
        anyhow::ensure!(
            before.dev() == after.dev() && before.ino() == after.ino(),
            "缓存目录在写入期间发生变化"
        );
        renameat(&cache, temporary.as_str(), &cache, path.file_name().unwrap())?;
        cache.sync_all()?;
        root.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = unlinkat(&cache, temporary.as_str(), UnlinkatFlags::NoRemoveDir);
    }
    result
}
