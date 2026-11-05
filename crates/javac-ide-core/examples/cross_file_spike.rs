
//! Cross-file spike: definition of a class declared in another analyzed file.

use std::path::PathBuf;

use javac_analysis_api::{AnalysisHost, Position};
use javac_ide_core::analysis::{AnalysisConfig, AnalysisSession};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let class_dir = PathBuf::from(&args[1]);
    let gson_source = PathBuf::from(&args[2]);
    let out_dir = PathBuf::from(&args[3]);
    let mut session = AnalysisSession::new(AnalysisConfig {
        class_path: vec![class_dir],
        output_dir: out_dir,
        source_level: "17".to_string(),
    });
    let host: &mut dyn AnalysisHost = &mut session;
    // 1. Analyze the declaration file (fills the project index).
    let first = host.analyze(&gson_source, None);
    println!("first errors={}", first.len());
    // 2. Analyze a second buffer that references the class.
    let second_path = gson_source.with_file_name("UsesGson.java");
    let text = "class UsesGson { Gson g; }\n";
    let second = host.analyze(&second_path, Some(text));
    println!("second errors={}", second.len());
    // 3. Definition of the reference must point into the first file.
    let definition = host.definition(&second_path, Position::new(0, 18));
    println!("definition: {:?}", definition);
}
