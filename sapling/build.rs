fn main() {
    tonic_build::compile_protos("proto/service.proto").unwrap();
    tonic_build::configure()
        .out_dir("src/generated")
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile(&["proto/api.proto"], &["proto"])
        .unwrap();
}
