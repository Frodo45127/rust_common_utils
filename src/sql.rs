
use getset::{Getters, Setters};
use anyhow::Result;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

#[derive(Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct Preset {
    script: String,
    params: HashMap<String, String>,
}

#[derive(Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct SQLScript {
    metadata: Metadata,
    queries: String,
}

#[derive(Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct Metadata {
    key: String,
    name: String,
    description: String,
    parameters: Vec<Param>,
    tables_affected: Vec<String>,
    replacements: HashMap<String, String>
}

#[derive(Getters, Setters, Serialize, Deserialize)]
#[getset(get = "pub", set = "pub")]
pub struct Param {
    key: String,
    name: String,
    r#type: ParamType,
    default_value: String,
}

#[derive(Serialize, Deserialize)]
pub enum ParamType {
    Bool,
    Integer,
    Float,
}

//---------------------------------------------------------------------------//
//                           Implementations
//---------------------------------------------------------------------------//

impl Preset {
    pub fn read(data: &[u8]) -> Result<Self> {
        Ok(serde_yml::from_slice(data)?)
    }
}

impl SQLScript {
    pub fn read(data: &[u8]) -> Result<Self> {
        Ok(serde_yml::from_slice(data)?)
    }

    pub fn prepare(&self, param_values: HashMap<String, String>) -> String {
        let mut script = self.queries.to_owned();

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
