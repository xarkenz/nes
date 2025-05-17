use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, Write};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

pub trait StateComponent {
    fn pull_state(&self) -> StateValue;

    fn push_state(&mut self, state: &StateValue) -> Result<(), String>;
}

#[derive(Debug)]
pub enum StateValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<StateValue>),
    LargeBuffer(Vec<u8>),
    Table(StateTable),
}

impl StateValue {
    const REF_TABLE_ENTRY: &'static str = "__REF";

    fn from_toml(toml: toml::Value, load_buffer: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>) -> Result<Self, String> {
        match toml {
            toml::Value::String(value) => Ok(StateValue::String(value)),
            toml::Value::Integer(value) => Ok(StateValue::Integer(value)),
            toml::Value::Float(value) => Ok(StateValue::Float(value)),
            toml::Value::Boolean(value) => Ok(StateValue::Boolean(value)),
            toml::Value::Array(array) => Ok(StateValue::Array(Result::from_iter(array
                .into_iter()
                .map(|value| StateValue::from_toml(value, load_buffer)))?)),
            toml::Value::Table(table) => {
                if let Some(toml::Value::String(file_name)) = table.get(Self::REF_TABLE_ENTRY) {
                    Ok(StateValue::LargeBuffer(load_buffer(file_name)?))
                }
                else {
                    Ok(StateValue::Table(Result::from_iter(table
                        .into_iter()
                        .map(|(key, value)| StateValue::from_toml(value, load_buffer).map(|value| (key, value))))?))
                }
            }
            _ => Err("Unsupported data type".to_string())
        }
    }

    fn to_toml<'a>(&'a self, name: Option<&str>, save_buffer: &mut dyn FnMut(&str, &'a [u8]) -> Result<String, String>) -> Result<toml::Value, String> {
        match self {
            StateValue::String(value) => Ok(toml::Value::String(value.clone())),
            StateValue::Integer(value) => Ok(toml::Value::Integer(*value)),
            StateValue::Float(value) => Ok(toml::Value::Float(*value)),
            StateValue::Boolean(value) => Ok(toml::Value::Boolean(*value)),
            StateValue::Array(array) => Ok(toml::Value::Array(Result::from_iter(array
                .iter()
                .map(|value| value.to_toml(name, save_buffer)))?)),
            StateValue::LargeBuffer(buffer) => {
                let name = name.unwrap_or("buffer");
                Ok(toml::Value::Table(toml::Table::from_iter([
                    (Self::REF_TABLE_ENTRY.into(), toml::Value::String(save_buffer(name, buffer)?)),
                ])))
            }
            StateValue::Table(table) => Ok(toml::Value::Table(Result::from_iter(table
                .iter()
                .map(|(key, value)| value.to_toml(Some(key), save_buffer).map(|value| (key.clone(), value))))?)),
        }
    }
}

pub trait StateValueMap {
    fn get(&self, key: &str) -> Option<&StateValue>;

    fn get_string(&self, key: &str) -> Result<&str, String> {
        match self.get(key) {
            Some(StateValue::String(value)) => Ok(value),
            _ => Err(format!("'{key}' must be a string"))
        }
    }

    fn get_integer(&self, key: &str) -> Result<i64, String> {
        match self.get(key) {
            Some(StateValue::Integer(value)) => Ok(*value),
            _ => Err(format!("'{key}' must be an integer"))
        }
    }

    fn get_float(&self, key: &str) -> Result<f64, String> {
        match self.get(key) {
            Some(StateValue::Float(value)) => Ok(*value),
            _ => Err(format!("'{key}' must be a float"))
        }
    }

    fn get_boolean(&self, key: &str) -> Result<bool, String> {
        match self.get(key) {
            Some(StateValue::Boolean(value)) => Ok(*value),
            _ => Err(format!("'{key}' must be a boolean"))
        }
    }

    fn get_array(&self, key: &str) -> Result<&[StateValue], String> {
        match self.get(key) {
            Some(StateValue::Array(value)) => Ok(value),
            _ => Err(format!("'{key}' must be an array"))
        }
    }

    fn get_large_buffer(&self, key: &str) -> Result<&[u8], String> {
        match self.get(key) {
            Some(StateValue::LargeBuffer(value)) => Ok(value),
            _ => Err(format!("'{key}' must be a large buffer reference"))
        }
    }

    fn get_table(&self, key: &str) -> Result<&StateTable, String> {
        match self.get(key) {
            Some(StateValue::Table(value)) => Ok(value),
            _ => Err(format!("'{key}' must be a table"))
        }
    }
}

pub type StateTable = HashMap<String, StateValue>;

impl StateValueMap for StateTable {
    fn get(&self, key: &str) -> Option<&StateValue> {
        self.get(key)
    }
}

pub struct StateArchive {
    state: StateValue,
}

impl StateArchive {
    const STATE_FILE_NAME: &'static str = "state.toml";

    pub fn new(state: StateValue) -> Self {
        Self { state }
    }
    
    pub fn state(&self) -> &StateValue {
        &self.state
    }

    pub fn load(reader: impl Read + Seek) -> Result<Self, String> {
        let mut zip_archive = ZipArchive::new(reader).map_err(|error| error.to_string())?;

        let mut toml_string = String::new();
        {
            let mut state_file = zip_archive.by_name(Self::STATE_FILE_NAME)
                .map_err(|error| error.to_string())?;
            state_file.read_to_string(&mut toml_string)
                .map_err(|error| error.to_string())?;
        }
        let toml = toml_string.parse::<toml::Value>()
            .map_err(|error| error.to_string())?;

        let state = StateValue::from_toml(toml, &mut |file_name| {
            let mut buffer_file = zip_archive.by_name(file_name)
                .map_err(|error| error.to_string())?;
            let mut buffer = Vec::new();
            buffer_file.read_to_end(&mut buffer)
                .map_err(|error| error.to_string())?;

            Ok(buffer)
        })?;

        Ok(Self::new(state))
    }

    pub fn save(&self, writer: impl Write + Seek) -> Result<(), String> {
        let mut zip_writer = ZipWriter::new(writer);
        let mut buffer_file_names = HashSet::new();

        let toml = self.state.to_toml(None, &mut |name, buffer| {
            for version in 0_u32.. {
                let file_name = format!("{name}.{version:x}.dat");
                if !buffer_file_names.contains(&file_name) {
                    buffer_file_names.insert(file_name.clone());

                    zip_writer.start_file(&file_name, SimpleFileOptions::default())
                        .map_err(|error| error.to_string())?;
                    zip_writer.write_all(buffer)
                        .map_err(|error| error.to_string())?;

                    return Ok(file_name);
                }
            }

            Ok("null".to_string())
        })?;

        zip_writer.start_file(Self::STATE_FILE_NAME, SimpleFileOptions::default())
            .map_err(|error| error.to_string())?;
        write!(zip_writer, "{toml}")
            .map_err(|error| error.to_string())?;
        zip_writer.finish()
            .map_err(|error| error.to_string())?;

        Ok(())
    }
}
