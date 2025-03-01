use rand::distr::Alphanumeric;
use rand::{rng, Rng};

pub fn make_id(prefix: &str) -> String {
    let r: String = rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    format!("{}-{}", prefix, r)
}
