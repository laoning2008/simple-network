use prost_build::Config;
fn main() {
    Config::new()
        .out_dir("src/")
        .compile_protos(&["../proto/hub_proto.proto"], &["../proto/"])
        .unwrap();
}