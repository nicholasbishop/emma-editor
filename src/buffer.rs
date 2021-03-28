use {
    crate::util,
    anyhow::Error,
    fehler::throws,
    ropey::Rope,
    std::{
        fs, io,
        path::{Path, PathBuf},
    },
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BufferId(String);

impl BufferId {
    fn new() -> BufferId {
        BufferId(util::make_id("buffer"))
    }
}

pub struct Buffer {
    id: BufferId,

    text: Rope,
    path: PathBuf,
}

impl Buffer {
    #[throws]
    pub fn from_path(path: &Path) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        Buffer {
            id: BufferId::new(),
            text,
            path: path.into(),
        }
    }

    pub fn id(&self) -> &BufferId {
        &self.id
    }
}
