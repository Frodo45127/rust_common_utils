//! This module contains the code for writing self-updateable tools published on Github.
//!
//! This should be moved to a separate crate once it's stable.

use anyhow::{anyhow, Result};
use itertools::Itertools;
use self_update::{backends::github::ReleaseList, cargo_crate_version, Download, get_target, Move, update::Release};
use tempfile::Builder;

use std::env::current_exe;
use std::fmt::Display;
use std::fs::{DirBuilder, File};

use crate::files_from_subdir;

const UPDATE_EXTENSION: &str = "zip";

const UPDATE_FOLDER_PREFIX: &str = "updates";

const CHANGELOG_FILE: &str = "CHANGELOG.txt";

const STABLE: &str = "Stable";
const BETA: &str = "Beta";

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

#[derive(Debug)]
pub struct Updater {
    update_channel: UpdateChannel,
    repo_owner: String,
    repo_name: String,
}

/// This enum controls the channels through where RPFM will try to update.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum UpdateChannel {
    Stable,
    Beta
}

/// This enum controls the possible responses from the server when checking for RPFM updates.
#[derive(Debug)]
pub enum APIResponse {

    /// This means a beta update was found.
    NewBetaUpdate(String),

    /// This means a major stable update was found.
    NewStableUpdate(String),

    /// This means a minor stable update was found.
    NewUpdateHotfix(String),

    /// This means no update was found.
    NoUpdate,

    /// This means don't know if there was an update or not, because the version we got was invalid.
    UnknownVersion,
}

//---------------------------------------------------------------------------//
//                           Implementation of Updater
//---------------------------------------------------------------------------//

impl Updater {

    pub fn new(update_channel: UpdateChannel, repo_owner: &str, repo_name: &str) -> Self {
        Self {
            update_channel,
            repo_owner: repo_owner.to_owned(),
            repo_name: repo_name.to_owned(),
        }
    }

    /// This function takes care of checking for updates, and if found, downloading and installing them automatically.
    pub fn download(&self) -> Result<()> {
        let last_release = self.last_release()?;

        // Get the download for our architecture.
        let asset = last_release.asset_for(get_target(), None).ok_or_else(|| anyhow!("No download available for your architecture."))?;
        let mut tmp_path = std::env::current_exe().unwrap();
        tmp_path.pop();
        let tmp_dir = Builder::new()
            .prefix(UPDATE_FOLDER_PREFIX)
            .tempdir_in(tmp_path)?;

        DirBuilder::new().recursive(true).create(&tmp_dir)?;

        // Nested stuff, because this seems to have problems with creating his own files before using them.
        {
            let tmp_zip_path = tmp_dir.path().join(&asset.name);
            let tmp_zip = File::create(&tmp_zip_path)?;

            Download::from_url(&asset.download_url)
                .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse().unwrap())
                .download_to(&tmp_zip)?;

            // self_update extractor doesn't work. It fails on every-single-test I did. So we use another one.
            let tmp_zip = File::open(&tmp_zip_path)?;
            zip_extract::extract(tmp_zip, tmp_dir.path(), true).map_err(|_| anyhow!("There was an error while extracting the update. This means either I uploaded a broken file, or your download was incomplete. In any case, no changes have been done so… try again later."))?;
        }

        let mut dest_base_path = current_exe()?;
        dest_base_path.pop();

        for updated_file in &files_from_subdir(tmp_dir.path(), true)? {

            // Ignore the downloaded ZIP.
            if let Some(extension) = updated_file.extension() {
                if let Some(extension) = extension.to_str() {
                    if extension == UPDATE_EXTENSION {
                        continue;
                    }
                }
            }

            let mut tmp_file = updated_file.to_path_buf();
            tmp_file.set_file_name(format!("{}_replacement_tmp", updated_file.file_name().unwrap().to_str().unwrap()));

            // Fix for files in folders: we have to get the destination path with the folders included.
            let tmp_file_relative = updated_file.strip_prefix(tmp_dir.path()).unwrap();
            let dest_file = dest_base_path.join(tmp_file_relative);

            // Make sure the destination folder actually exists, or this will fail.
            let mut dest_folder = dest_base_path.join(tmp_file_relative);
            dest_folder.pop();
            DirBuilder::new().recursive(true).create(&dest_folder)?;

            Move::from_source(updated_file)
                .replace_using_temp(&tmp_file)
                .to_dest(&dest_file)?;
        }

        // Open the changelog because people don't read it.
        let changelog_path = dest_base_path.join(CHANGELOG_FILE);
        let _ = open::that(changelog_path);

        Ok(())
    }

    /// This function takes care of checking for new TWPatcher updates.
    pub fn check(&self) -> Result<APIResponse> {
        let last_release = self.last_release()?;

        let current_version = cargo_crate_version!().split('.').map(|x| x.parse::<i32>().unwrap_or(0)).collect::<Vec<i32>>();
        let last_version = &last_release.version.split('.').map(|x| x.parse::<i32>().unwrap_or(0)).collect::<Vec<i32>>();

        // Before doing anything else, check if we are going back to stable after a beta, and we are currently in a beta version.
        // In that case, return the last stable as valid.
        if let UpdateChannel::Stable = self.update_channel {
            if current_version[2] >= 99 {
                return Ok(APIResponse::NewStableUpdate(format!("v{}", last_version.iter().map(|x| x.to_string()).join("."))));
            }
        }

        // Get the version numbers from our version and from the latest released version, so we can compare them.
        let first = (last_version[0], current_version[0]);
        let second = (last_version[1], current_version[1]);
        let third = (last_version[2], current_version[2]);

        // If this is triggered, there has been a problem parsing the current/remote version.
        if first.0 == 0 && second.0 == 0 && third.0 == 0 || first.1 == 0 && second.1 == 0 && third.1 == 0 {
            Ok(APIResponse::UnknownVersion)
        }

        // If the current version is different than the last released version...
        else if last_version != &current_version {

            // If the latest released version is lesser than the current version...
            // No update. We are using a newer build than the last build released (dev?).
            if first.0 < first.1 { Ok(APIResponse::NoUpdate) }

            // If the latest released version is greater than the current version...
            // New major update. No more checks needed.
            else if first.0 > first.1 {
                match self.update_channel {
                    UpdateChannel::Stable => Ok(APIResponse::NewStableUpdate(format!("v{}", last_version.iter().map(|x| x.to_string()).join(".")))),
                    UpdateChannel::Beta => Ok(APIResponse::NewBetaUpdate(format!("v{}", last_version.iter().map(|x| x.to_string()).join(".")))),
                }
            }

            // If the latest released version the same than the current version, we check the second, then the third number.
            // No update. We are using a newer build than the last build released (dev?).
            else if second.0 < second.1 { Ok(APIResponse::NoUpdate) }

            // New major update. No more checks needed.
            else if second.0 > second.1 {
                match self.update_channel {
                    UpdateChannel::Stable => Ok(APIResponse::NewStableUpdate(format!("v{}", last_version.iter().map(|x| x.to_string()).join(".")))),
                    UpdateChannel::Beta => Ok(APIResponse::NewBetaUpdate(format!("v{}", last_version.iter().map(|x| x.to_string()).join(".")))),
                }
            }

            // We check the last number in the versions, and repeat. Scraping the barrel...
            // No update. We are using a newer build than the last build released (dev?).
            else if third.0 < third.1 { Ok(APIResponse::NoUpdate) }

            // If the latest released version only has the last number higher, is a hotfix.
            else if third.0 > third.1 {
                match self.update_channel {
                    UpdateChannel::Stable => Ok(APIResponse::NewUpdateHotfix(format!("v{}", last_version.iter().map(|x| x.to_string()).join(".")))),
                    UpdateChannel::Beta => Ok(APIResponse::NewBetaUpdate(format!("v{}", last_version.iter().map(|x| x.to_string()).join(".")))),
                }
            }

            // This means both are the same, and the checks will never reach this place thanks to the parent if.
            else { unreachable!() }
        }
        else {
            Ok(APIResponse::NoUpdate)
        }
    }

    /// This function returns the last release available, according to our update channel.
    pub fn last_release(&self) -> Result<Release> {
        let releases = ReleaseList::configure()
            .repo_owner(&self.repo_owner)
            .repo_name(&self.repo_name)
            .build()?
            .fetch()?;

        match releases.iter().find(|release| {
            match self.update_channel {
                UpdateChannel::Stable => release.version.split('.').collect::<Vec<&str>>()[2].parse::<i32>().unwrap_or(0) < 99,
                UpdateChannel::Beta => true
            }
        }) {
            Some(last_release) => Ok(last_release.clone()),
            None => Err(anyhow!("Failed to get last release (should never happen)."))
        }
    }
}

impl Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(match &self {
            UpdateChannel::Stable => STABLE,
            UpdateChannel::Beta => BETA,
        }, f)
    }
}

impl TryFrom<&str> for UpdateChannel {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            BETA => Ok(Self::Beta),
            STABLE => Ok(Self::Stable),
            _ => Err(anyhow!("Invalid update channel: {}", value))
        }
    }
}
