use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    path::Path,
};

use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use serde::{Deserialize, Serialize};
use serde_json::{de::IoRead, StreamDeserializer};

use crate::PersistentDbError;

#[derive(Serialize, Deserialize)]
pub(crate) struct BlockRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prefix: Option<JsonPointer>,
    pub(crate) patch_records: Vec<JsonPatch>,
}

#[derive(Copy, Clone, Serialize)]
pub(crate) struct BlockRecordRef<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prefix: Option<&'a JsonPointer>,
    pub(crate) patch_records: &'a [JsonPatch],
}

pub(crate) struct ActiveBlockFile {
    file: File,
    size: u64,
}

impl ActiveBlockFile {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, PersistentDbError> {
        let file = OpenOptions::new().append(true).create(true).open(path)?;
        let md = file.metadata()?;
        Ok(Self {
            file,
            size: md.len(),
        })
    }

    pub(crate) fn append(&mut self, record: BlockRecordRef<'_>) -> Result<(), PersistentDbError> {
        let data = serde_json::to_vec(&record)?;
        let data_len = data.len() as u64;
        if self.size + data_len > 1024 * 1024 * 256 {
            return Err(PersistentDbError::BlockFileIsFull);
        }
        self.file.write_all(&data)?;
        self.size += data_len;
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> Result<(), PersistentDbError> {
        Ok(self.file.flush()?)
    }
}

pub(crate) struct InactiveBlockFile {
    deserializer: StreamDeserializer<'static, IoRead<BufReader<File>>, BlockRecord>,
}

impl InactiveBlockFile {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, PersistentDbError> {
        let file = File::open(path)?;
        Ok(Self {
            deserializer: StreamDeserializer::new(IoRead::new(BufReader::new(file))),
        })
    }
}

impl Iterator for InactiveBlockFile {
    type Item = Result<BlockRecord, PersistentDbError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.deserializer.next().map(|res| res.map_err(Into::into))
    }
}
