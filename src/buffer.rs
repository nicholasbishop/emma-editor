use {
    anyhow::Error,
    fehler::throws,
    ropey::Rope,
    std::{
        fs, io,
        path::{Path, PathBuf},
    },
};

pub struct Buffer {
    text: Rope,
    path: PathBuf,
}

impl Buffer {
    #[throws]
    pub fn from_path(path: &Path) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        let buffer = Buffer {
            text,
            path: path.into(),
        };

        buffer
    }
}
