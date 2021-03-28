use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub fn make_id(prefix: &str) -> String {
    let r: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    format!("{}-{}", prefix, r)
}
