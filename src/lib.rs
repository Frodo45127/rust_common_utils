
use anyhow::{anyhow, Result};
use std::fs::read_dir;
use std::path::{Path, PathBuf};

pub mod updater;
pub mod sql;

/// This function retuns a `Vec<PathBuf>` containing all the files in the provided folder.
fn files_from_subdir(current_path: &Path, scan_subdirs: bool) -> Result<Vec<PathBuf>> {

    // Fast path. Takes a few ms less than the other one.
    if !scan_subdirs {
        return Ok(read_dir(current_path)?
            .flatten()
            .filter(|file| {
                if let Ok(metadata) = file.metadata() {
                    metadata.is_file()
                } else { false }
            })
            .map(|file| file.path()).collect());
    }

    // Slow path. Can scan subdirs.
    let mut file_list: Vec<PathBuf> = vec![];
    match read_dir(current_path) {
        Ok(files_in_current_path) => {
            for file in files_in_current_path {

                // Get his path and continue, or return an error if it can't be read.
                match file {
                    Ok(file) => {
                        let file_path = file.path();

                        // If it's a file, add it to the list.
                        if file_path.is_file() {
                            file_list.push(file_path);
                        }

                        // If it's a folder, add his files to the list.
                        else if file_path.is_dir() && scan_subdirs {
                            let mut subfolder_files_path = files_from_subdir(&file_path, scan_subdirs)?;
                            file_list.append(&mut subfolder_files_path);
                        }
                    }
                    Err(_) => return Err(anyhow!("Error reading file: {}", current_path.to_string_lossy().to_string())),
                }
            }
        }

        // In case of reading error, report it.
        Err(_) => return Err(anyhow!("Error reading folder: {}", current_path.to_string_lossy().to_string())),
    }

    // Return the list of paths.
    Ok(file_list)
}
