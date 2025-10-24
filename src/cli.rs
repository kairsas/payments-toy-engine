use std::env;

use color_eyre::eyre::{OptionExt, Result};

pub struct CliArgs {
    pub input_file_path: String,
}

impl CliArgs {
    pub fn load() -> Result<Self> {
        let args: Vec<String> = env::args().collect();

        let input_file_path = args.get(1).ok_or_eyre("Input file not passed")?.to_owned();

        Ok(CliArgs { input_file_path })
    }
}
