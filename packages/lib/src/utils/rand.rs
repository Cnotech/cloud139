use rand::distributions::{Alphanumeric, DistString};

pub fn generate_rand_str(len: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), len)
}
