use std::fs::{self, read_dir};

use protobuf_codegen::Codegen;

const OUT_DIR: &'static str = "./src/proto/";

fn main() {
    let files = read_dir("./proto").unwrap();
    let files = files.map(|v| format!("proto/{}", v.unwrap().file_name().into_string().unwrap()));

    read_dir(OUT_DIR).unwrap().into_iter().for_each(|v| {
        if let Ok(entry) = v {
            if entry.file_name().to_str().unwrap().ends_with(".rs") {
                fs::remove_file(entry.path()).unwrap();
            }
        }
    });

    Codegen::new()
        .inputs(files)
        .include("proto")
        .out_dir(OUT_DIR)
        .run()
        .unwrap();
}
