
use anyhow::Result;
use getset::{Getters, Setters};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

#[derive(Default, Debug, Clone, Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct Preset {
    key: String,
    name: String,
    script_key: String,
    params: HashMap<String, String>,
}

#[derive(Default, Debug, Clone, Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct SQLScript {
    metadata: Metadata,

    #[serde(skip)]
    queries: String,
}

#[derive(Default, Debug, Clone, Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct Metadata {
    key: String,
    name: String,
    description: String,
    parameters: Vec<Param>,
    tables_affected: Vec<String>,
    replacements: HashMap<String, String>
}

#[derive(Default, Debug, Clone, Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct Param {
    key: String,
    name: String,
    r#type: ParamType,
    default_value: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    #[default]
    Bool,
    Integer,
    Float,
}

//---------------------------------------------------------------------------//
//                           Implementations
//---------------------------------------------------------------------------//

impl Preset {
    pub fn read(path: &Path) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);
        let mut data = vec![];
        file.read_to_end(&mut data)?;

        Ok(serde_yml::from_slice(&data)?)
    }
}

impl SQLScript {

    pub fn from_path(path: &Path) -> Result<Self> {
        let mut script = Self::default();

        script.read_metadata(&path)?;
        script.read_queries(&path)?;

        Ok(script)
    }

    pub fn read_metadata(&mut self, path: &Path) -> Result<()> {
        let mut file = BufReader::new(File::open(path)?);
        let mut data = vec![];
        file.read_to_end(&mut data)?;

        dbg!(String::from_utf8(data.clone()));
        self.metadata = serde_yml::from_slice(&data)?;

        Ok(())
    }

    pub fn read_queries(&mut self, path: &Path) -> Result<()> {
        let mut sql_file = path.to_path_buf();
        sql_file.set_file_name(self.metadata().key());
        sql_file.set_extension("sql");

        let mut file = BufReader::new(File::open(sql_file)?);
        file.read_to_string(&mut self.queries)?;

        Ok(())
    }

    pub fn prepare(&self, param_values: HashMap<String, String>) -> String {
        let mut script = self.queries.replace("\r\n", "\n");

        // First apply the string replacements. To support nested replacements... we do some magic.
        let mut replacements = self.metadata.replacements.clone();
        let limit = 30;
        let mut cycle = 0;
        loop {
            let mut done = true;
            let replacements_copy = replacements.clone();
            for (key_mut, value_mut) in &mut replacements {
                for (key, value) in &replacements_copy {
                    if key != key_mut && value_mut.contains(key) {
                        *value_mut = value_mut.replace(key, value);
                        done = false;
                    }
                }
            }

            if done || cycle == limit {
                break;
            }

            cycle += 1;
        }

        for (key, value) in &replacements {
            script = script.replace(key, value);
        }

        // Then, apply the params.
        for (key, value) in &param_values {
            script = script.replace(key, value);
        }

        script
    }
}
