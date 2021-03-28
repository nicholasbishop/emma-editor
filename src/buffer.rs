use {
    anyhow::Error,
    fehler::throws,
    rand::{distributions::Alphanumeric, thread_rng, Rng},
    ropey::Rope,
    std::{
        fs, io,
        path::{Path, PathBuf},
    },
};

pub struct BufferId(String);

impl BufferId {
    fn new() -> BufferId {
        let r: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();
        BufferId(format!("buffer-{}", r))
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
}
