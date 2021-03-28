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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BufferId(String);

impl BufferId {
    pub fn new() -> BufferId {
        BufferId(util::make_id("buffer"))
    }
}

pub struct Buffer {
    text: Rope,
    path: PathBuf,
}

impl Buffer {
    #[throws]
    pub fn from_path(path: &Path) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        Buffer {
            text,
            path: path.into(),
        }
    }
}
