//! Проверка и автообновление из GitHub Releases.

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const GITHUB_OWNER: &str = "fgbm";
const GITHUB_REPO: &str = "telemt-admin";
const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Имя asset для Linux x86_64.
const ASSET_LINUX_X86_64: &str = "telemt-admin-linux-x86_64.tar.gz";
const ASSET_LINUX_X86_64_SHA256: &str = "telemt-admin-linux-x86_64.tar.gz.sha256";

/// Имя asset для Windows x86_64.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const ASSET_WINDOWS_X86_64: &str = "telemt-admin-windows-x86_64.zip";

fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| Version::parse("0.0.0").expect("fallback version"))
}

fn parse_tag_version(tag: &str) -> Option<Version> {
    let v = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(v).ok()
}

/// Возвращает имя asset для текущей платформы (если поддерживается).
fn current_platform_asset() -> Option<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    return Some(ASSET_LINUX_X86_64);

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Some(ASSET_WINDOWS_X86_64);

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    return None;
}

/// Поддерживается ли self-update на текущей платформе.
fn is_self_update_supported() -> bool {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    return true;

    #[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
    return false;
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    let url = format!(
        "{}/repos/{}/{}/releases/latest",
        GITHUB_API_BASE, GITHUB_OWNER, GITHUB_REPO
    );
    let client = reqwest::Client::builder()
        .user_agent(format!("telemt-admin/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Создание HTTP-клиента")?;
    let release: GitHubRelease = client
        .get(&url)
        .send()
        .await
        .context("Запрос GitHub API")?
        .error_for_status()
        .context("GitHub API вернул ошибку")?
        .json()
        .await
        .context("Парсинг ответа GitHub API")?;
    Ok(release)
}

/// Проверяет наличие новой версии и выводит результат в stdout/stderr.
pub async fn run_check_update() -> Result<()> {
    let current = current_version();

    let release = match fetch_latest_release().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Ошибка проверки обновлений: {}", e);
            return Err(e);
        }
    };

    let latest = match parse_tag_version(&release.tag_name) {
        Some(v) => v,
        None => {
            eprintln!("Не удалось распарсить версию из тега: {}", release.tag_name);
            return Ok(());
        }
    };

    println!("Текущая версия: {}", current);
    println!("Последняя версия: {}", latest);

    if latest > current {
        println!("\nДоступна новая версия {}!", latest);
        if let Some(asset_name) = current_platform_asset()
            && let Some(asset) = release.assets.iter().find(|a| a.name == asset_name)
        {
            println!("Скачать: {}", asset.browser_download_url);
        }
        if is_self_update_supported() {
            println!("Для автообновления выполните: telemt-admin self-update");
        } else {
            println!(
                "Автообновление на данной платформе не поддерживается. Скачайте бинарник вручную."
            );
        }
    } else {
        println!("\nУстановлена актуальная версия.");
    }

    Ok(())
}

/// Обновляет бинарник до последней версии (только Linux x86_64).
pub async fn run_self_update() -> Result<()> {
    if !is_self_update_supported() {
        anyhow::bail!(
            "Автообновление поддерживается только на Linux x86_64 (gnu). \
             Текущая платформа: {} {}. \
             Скачайте обновление вручную с https://github.com/{}/{}/releases",
            std::env::consts::OS,
            std::env::consts::ARCH,
            GITHUB_OWNER,
            GITHUB_REPO
        );
    }

    let release = fetch_latest_release().await?;
    let current = current_version();
    let latest = parse_tag_version(&release.tag_name)
        .context("Не удалось распарсить версию из тега релиза")?;
    if latest <= current {
        println!(
            "Установлена актуальная версия {}. Обновление не требуется.",
            current
        );
        return Ok(());
    }

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == ASSET_LINUX_X86_64)
        .context("Asset для Linux x86_64 не найден в релизе")?;

    let sha_asset = release
        .assets
        .iter()
        .find(|a| a.name == ASSET_LINUX_X86_64_SHA256)
        .context("SHA-256 checksum для Linux x86_64 не найден в релизе")?;

    let current_exe = std::env::current_exe().context("Не удалось определить путь к бинарнику")?;
    let exe_dir = current_exe
        .parent()
        .context("Некорректный путь к бинарнику")?;

    let client = reqwest::Client::builder()
        .user_agent(format!("telemt-admin/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Создание HTTP-клиента")?;

    println!("Скачивание {}...", sha_asset.browser_download_url);
    let sha256_text = client
        .get(&sha_asset.browser_download_url)
        .send()
        .await
        .context("Запрос SHA-256 файла")?
        .error_for_status()
        .context("Ошибка загрузки SHA-256 файла")?
        .text()
        .await
        .context("Чтение SHA-256 файла")?;

    let expected_hash = sha256_text
        .split_whitespace()
        .next()
        .context("Некорректный формат SHA-256 файла")?;

    println!("Скачивание {}...", asset.browser_download_url);
    let archive_bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .context("Запрос архива")?
        .error_for_status()
        .context("Ошибка загрузки архива")?
        .bytes()
        .await
        .context("Чтение тела ответа")?;

    let archive_bytes = archive_bytes.to_vec();
    let expected_hash = expected_hash.to_string();
    let verified_bytes = tokio::task::spawn_blocking(move || {
        let mut hasher = Sha256::new();
        hasher.update(&archive_bytes);
        let actual_hash = format!("{:x}", hasher.finalize());
        if actual_hash != expected_hash {
            anyhow::bail!(
                "Контрольная сумма архива не совпадает. Обновление отменено из соображений безопасности."
            );
        }
        Ok::<_, anyhow::Error>(archive_bytes)
    })
    .await
    .context("Blocking SHA-256 verification task failed")??;

    install_downloaded_release(
        exe_dir.to_path_buf(),
        current_exe.clone(),
        verified_bytes,
    )
    .await?;

    println!(
        "Обновление до версии {} завершено. Перезапустите сервис: systemctl restart telemt-admin.service",
        latest
    );
    Ok(())
}

async fn install_downloaded_release(
    exe_dir: PathBuf,
    current_exe: PathBuf,
    archive_bytes: Vec<u8>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        install_downloaded_release_blocking(&exe_dir, &current_exe, &archive_bytes)
    })
    .await
    .context("Blocking self-update task failed")?
}

fn install_downloaded_release_blocking(
    exe_dir: &Path,
    current_exe: &Path,
    archive_bytes: &[u8],
) -> Result<()> {
    let mut archive = flate2::read::GzDecoder::new(archive_bytes);
    let mut tar = tar::Archive::new(&mut archive);
    let temp_dir = tempfile::tempdir_in(exe_dir)
        .context("Не удалось создать временную директорию (проверьте права на запись)")?;
    tar.unpack(temp_dir.path()).context("Распаковка архива")?;

    let extracted_binary = temp_dir.path().join("telemt-admin");
    if !extracted_binary.exists() {
        anyhow::bail!("В архиве не найден файл telemt-admin");
    }

    let new_binary_path = exe_dir.join(".telemt-admin.new");
    fs::copy(&extracted_binary, &new_binary_path).context("Копирование нового бинарника")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&new_binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&new_binary_path, perms)?;
    }

    fs::rename(&new_binary_path, current_exe).context(
        "Не удалось заменить бинарник. Убедитесь, что у процесса есть права на запись в директорию с исполняемым файлом.",
    )?;

    Ok(())
}
