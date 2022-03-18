use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use memdb::MemDb;

use crate::{
    block_file::{ActiveBlockFile, BlockRecordRef, InactiveBlockFile},
    PersistentDbError,
};

const SNAPSHOT_FILE_NAME: &str = "snapshot.data";
const TEMP_SNAPSHOT_FILE_NAME: &str = "snapshot.temp";

pub struct PersistentDb {
    path: PathBuf,
    active_block: Option<(usize, ActiveBlockFile)>,
    compacting: Arc<AtomicBool>,
}

impl PersistentDb {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, PersistentDbError> {
        let path = path.into();
        std::fs::create_dir_all(&path)?;
        let blocks = get_block_list(&path)?;

        let active_block = match blocks.last().copied() {
            Some(index) => {
                let active_block = ActiveBlockFile::open(path.join(format!("{}.block", index)))?;
                Some((index, active_block))
            }
            None => None,
        };

        Ok(Self {
            path,
            active_block,
            compacting: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn create_memdb(&self) -> Result<MemDb, PersistentDbError> {
        tracing::info!(path = %self.path.display(), "load data from persistentdb");
        load_memdb(&self.path, &get_block_list(&self.path)?)
    }

    pub fn append(
        &mut self,
        prefix: Option<&JsonPointer>,
        patch_records: &[JsonPatch],
        flush: bool,
    ) -> Result<(), PersistentDbError> {
        let record = BlockRecordRef {
            prefix,
            patch_records,
        };

        let new_index = match &mut self.active_block {
            Some((index, block)) => match block.append(record) {
                Ok(()) => {
                    if flush {
                        block.flush()?;
                    }
                    return Ok(());
                }
                Err(PersistentDbError::BlockFileIsFull) => {
                    // create a new block file
                    *index + 1
                }
                Err(err) => return Err(err),
            },
            None => 1,
        };

        let mut block_file = ActiveBlockFile::open(self.path.join(format!("{}.block", new_index)))?;
        block_file.append(record)?;
        if flush {
            block_file.flush()?;
        }
        self.active_block = Some((new_index, block_file));
        Ok(())
    }

    pub fn compact(&self) {
        if self
            .compacting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire)
            .is_ok()
        {
            let compacting = self.compacting.clone();
            let path = self.path.clone();
            std::thread::spawn(move || {
                if let Err(err) = do_compact(&path) {
                    tracing::error!(error = %err, "failed to compact data");
                }
                compacting.store(false, Ordering::SeqCst);
            });
        }
    }
}

fn get_block_list(path: &Path) -> Result<Vec<usize>, PersistentDbError> {
    let read_dir = path.read_dir()?;
    let mut blocks = Vec::new();

    for res in read_dir {
        let entry = res?;
        let file_path = entry.path();
        if file_path.extension().and_then(|ext| ext.to_str()) == Some("block") {
            if let Some(index) = file_path
                .file_stem()
                .and_then(|name| name.to_str())
                .and_then(|name| name.parse::<usize>().ok())
            {
                blocks.push(index);
            }
        }
    }

    blocks.sort_unstable();
    Ok(blocks)
}

fn load_memdb(path: &Path, blocks: &[usize]) -> Result<MemDb, PersistentDbError> {
    let snapshot_path = path.join(SNAPSHOT_FILE_NAME);
    let mut db = if snapshot_path.exists() {
        let value = serde_json::from_slice(&std::fs::read(snapshot_path)?)?;
        MemDb::new(value)
    } else {
        MemDb::default()
    };

    for block_id in blocks {
        for res in InactiveBlockFile::open(path.join(format!("{}.block", block_id)))? {
            let record = res?;
            db.patch(record.prefix.as_ref(), record.patch_records)?;
        }
    }

    Ok(db)
}

fn do_compact(path: &Path) -> Result<(), PersistentDbError> {
    let blocks = get_block_list(path)?;

    if blocks.len() > 5 {
        let blocks = &blocks[..blocks.len() - 1];
        let db = load_memdb(path, blocks)?;
        let now = Instant::now();

        tracing::info!(blocks = ?blocks, "compact start");

        let data = serde_json::to_vec(db.root())?;
        std::fs::write(path.join(TEMP_SNAPSHOT_FILE_NAME), data)?;
        std::fs::rename(
            path.join(TEMP_SNAPSHOT_FILE_NAME),
            path.join(SNAPSHOT_FILE_NAME),
        )?;

        tracing::info!(
            elapsed_seconds = now.elapsed().as_secs_f32(),
            "compact finish"
        );
    }

    Ok(())
}
